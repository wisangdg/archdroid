#!/usr/bin/env bash
# Disposable Arch test VM for the P0-8 live validation matrix.
#
# Everything lives outside the repo in $VM_DIR (default ~/archdroid-test-vm):
#   Arch-Linux-x86_64-cloudimg.qcow2  official base image (read-only)
#   working.qcow2                     provisioning overlay
#   baseline.qcow2                    clean snapshot after provisioning
#   runs/<name>.qcow2                 disposable overlay per test run
#
# Usage:
#   scripts/test-vm/vm.sh provision   # download, cloud-init, boot, wait for SSH
#   scripts/test-vm/vm.sh ssh         # shell into the provisioning VM
#   scripts/test-vm/vm.sh stop        # graceful shutdown of the provisioning VM
#   scripts/test-vm/vm.sh snapshot    # commit working.qcow2 into baseline.qcow2
#   scripts/test-vm/vm.sh run [name]  # boot a fresh overlay from baseline.qcow2
#   scripts/test-vm/vm.sh ssh-run [name]
#   scripts/test-vm/vm.sh stop-run [name]
#   scripts/test-vm/vm.sh status
set -euo pipefail

VM_DIR="${VM_DIR:-$HOME/archdroid-test-vm}"
IMAGE_URL="${IMAGE_URL:-https://geo.mirror.pkgbuild.com/images/latest/Arch-Linux-x86_64-cloudimg.qcow2}"
IMAGE_SHA256="${IMAGE_SHA256:-08754e28d91b1680086c6211ef4df13559e766fff7cdb4006489b14b5d281f08}"
BASE="$VM_DIR/Arch-Linux-x86_64-cloudimg.qcow2"
WORK="$VM_DIR/working.qcow2"
BASELINE="$VM_DIR/baseline.qcow2"
SEED="$VM_DIR/seed.iso"
KEY="$VM_DIR/id_ed25519"
SSH_USER="tester"
PROVISION_PORT="${SSH_PORT:-2222}"
RUN_PORT="${RUN_PORT:-2223}"
SSH_OPTS=(-o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR -o ConnectTimeout=5)
QEMU=(qemu-system-x86_64 -machine q35,accel=kvm -cpu host -smp 4 -m 6144 -display none)

die() { echo "error: $*" >&2; exit 1; }

require() {
  for command in "$@"; do
    command -v "$command" >/dev/null || die "missing host tool: $command"
  done
}

start_qemu() {
  local disk="$1" seed="$2" port="$3" pidfile="$4" serial="$5" name="$6"
  [[ -e "$pidfile" ]] && kill -0 "$(cat "$pidfile")" 2>/dev/null && die "VM already running ($pidfile)"
  ss -ltn | grep -q ":$port " && die "TCP port $port is already in use; set SSH_PORT/RUN_PORT"

  "${QEMU[@]}" \
    -name "$name" \
    -drive "file=$disk,if=virtio,format=qcow2,cache=writeback" \
    -drive "file=$seed,media=cdrom,readonly=on" \
    -netdev "user,id=net0,hostfwd=tcp:127.0.0.1:$port-:22" \
    -device virtio-net-pci,netdev=net0 \
    -object rng-random,filename=/dev/urandom,id=rng0 \
    -device virtio-rng-pci,rng=rng0 \
    -serial "file:$serial" \
    -pidfile "$pidfile" \
    -daemonize
}

wait_for_ssh() {
  local port="$1" deadline=$((SECONDS + 900))
  printf 'waiting for SSH on 127.0.0.1:%s ...\n' "$port"
  while (( SECONDS < deadline )); do
    if ssh "${SSH_OPTS[@]}" -i "$KEY" -p "$port" "$SSH_USER@127.0.0.1" true 2>/dev/null; then
      echo "SSH ready"
      return 0
    fi
    sleep 5
  done
  die "SSH did not come up within 15 minutes; inspect the serial log"
}

wait_for_provisioned() {
  local port="$1" deadline=$((SECONDS + 1800))
  printf 'waiting for cloud-init to finish ...\n'
  while (( SECONDS < deadline )); do
    if ssh "${SSH_OPTS[@]}" -i "$KEY" -p "$port" "$SSH_USER@127.0.0.1" \
      'test -f "$HOME/PROVISIONED"' 2>/dev/null; then
      echo "cloud-init finished"
      return 0
    fi
    sleep 5
  done
  die "cloud-init did not finish within 30 minutes; inspect the serial log"
}

stage_cloud_init() {
  ssh-keygen -q -t ed25519 -N '' -C archdroid-test-vm -f "$KEY" <<<y >/dev/null 2>&1 || true
  chmod 600 "$KEY"
  cat > "$VM_DIR/meta-data" <<EOF
instance-id: archdroid-test-$(date +%Y%m%d%H%M%S)
local-hostname: archdroid-test
EOF
  cat > "$VM_DIR/user-data" <<EOF
#cloud-config
hostname: archdroid-test
preserve_hostname: false
ssh_pwauth: false
disable_root: true
users:
  - name: $SSH_USER
    groups: [wheel]
    shell: /bin/bash
    lock_passwd: true
    sudo: "ALL=(ALL) NOPASSWD:ALL"
    ssh_authorized_keys:
      - $(cat "$KEY.pub")
write_files:
  # The stock Arch kernel uses the Rust binder driver plus binderfs; there is
  # no binder_linux module. Waydroid mounts binderfs and allocates the device
  # nodes itself, so no modules-load/modprobe configuration is needed.
  - path: /usr/local/bin/wayland-headless
    permissions: "0755"
    content: |
      #!/bin/sh
      set -eu
      : "\${XDG_RUNTIME_DIR:=/run/user/\$(id -u)}"
      export XDG_RUNTIME_DIR
      mkdir -p "\$XDG_RUNTIME_DIR"
      chmod 700 "\$XDG_RUNTIME_DIR"
      exec weston --backend=headless-backend.so --socket="\${WAYLAND_DISPLAY:-wayland-0}" --idle-time=0
runcmd:
  - [bash, -lc, "pacman -Syu --noconfirm --needed"]
  - [bash, -lc, "pacman -S --noconfirm --needed waydroid weston polkit android-tools git python squashfs-tools lzip xdg-utils alacritty dbus curl"]
  - [bash, -lc, "grep -qw binder /proc/filesystems || { echo 'binderfs missing' >&2; exit 1; }"]
  - [bash, -lc, "systemctl enable --now sshd"]
  - [bash, -lc, "install -d -o $SSH_USER -g $SSH_USER /home/$SSH_USER/test-data"]
  - [bash, -lc, "printf 'this is not an APK\\\\n' > /home/$SSH_USER/test-data/invalid.apk"]
  - [bash, -lc, "curl -L --fail --retry 3 -o /home/$SSH_USER/test-data/F-Droid.apk https://f-droid.org/F-Droid.apk || true"]
  - [bash, -lc, "chown -R $SSH_USER:$SSH_USER /home/$SSH_USER/test-data; touch /home/$SSH_USER/PROVISIONED"]
EOF
  rm -f "$SEED"
  ( cd "$VM_DIR" && xorriso -as mkisofs -output "$SEED" -volid cidata -joliet -rock meta-data user-data >/dev/null )
}

cmd_provision() {
  require qemu-system-x86_64 qemu-img xorriso ssh ssh-keygen ss
  mkdir -p "$VM_DIR"
  if [[ ! -f "$BASE" ]]; then
    echo "downloading $IMAGE_URL"
    curl -L --fail --retry 3 --connect-timeout 20 -o "$BASE.part" "$IMAGE_URL"
    mv "$BASE.part" "$BASE"
  fi
  echo "$IMAGE_SHA256  $BASE" | sha256sum -c -
  stage_cloud_init
  [[ -f "$WORK" ]] || qemu-img create -q -f qcow2 -F qcow2 -b "$BASE" "$WORK" 40G
  start_qemu "$WORK" "$SEED" "$PROVISION_PORT" "$VM_DIR/qemu.pid" "$VM_DIR/serial.log" archdroid-test
  wait_for_ssh "$PROVISION_PORT"
  wait_for_provisioned "$PROVISION_PORT"
  ssh "${SSH_OPTS[@]}" -i "$KEY" -p "$PROVISION_PORT" "$SSH_USER@127.0.0.1" \
    'waydroid --version && grep -qw binder /proc/filesystems && echo PROVISION_OK'
}

cmd_ssh() {
  exec ssh "${SSH_OPTS[@]}" -i "$KEY" -p "$PROVISION_PORT" "$SSH_USER@127.0.0.1" "$@"
}

cmd_stop() {
  ssh "${SSH_OPTS[@]}" -i "$KEY" -p "$PROVISION_PORT" "$SSH_USER@127.0.0.1" 'sudo poweroff' || true
  for _ in $(seq 1 30); do
    [[ -e "$VM_DIR/qemu.pid" ]] || break
    kill -0 "$(cat "$VM_DIR/qemu.pid")" 2>/dev/null || break
    sleep 1
  done
  rm -f "$VM_DIR/qemu.pid"
}

cmd_snapshot() {
  [[ -f "$WORK" ]] || die "no working.qcow2; run provision first"
  [[ ! -e "$BASELINE" ]] || die "baseline.qcow2 already exists; remove it explicitly to re-snapshot"
  cmd_stop
  qemu-img convert -p -O qcow2 "$WORK" "$BASELINE"
  sha256sum "$BASELINE" > "$BASELINE.sha256"
  qemu-img info "$BASELINE" | sed -n '1,8p'
}

run_pidfile() { echo "$VM_DIR/runs/$1.pid"; }

cmd_run() {
  local name="${1:-run-$(date +%Y%m%d-%H%M%S)}"
  local base="${BASE_IMAGE:-$BASELINE}"
  local disk="$VM_DIR/runs/$name.qcow2"
  [[ -f "$base" ]] || die "no baseline image at $base; run snapshot first or set BASE_IMAGE"
  mkdir -p "$VM_DIR/runs"
  [[ -f "$disk" ]] || qemu-img create -q -f qcow2 -F qcow2 -b "$base" "$disk"
  start_qemu "$disk" "$SEED" "$RUN_PORT" "$(run_pidfile "$name")" "$VM_DIR/runs/$name.serial.log" "archdroid-test-$name"
  wait_for_ssh "$RUN_PORT"
  echo "run '$name' is disposable; delete $disk to discard it"
}

cmd_ssh_run() {
  local name="${1:?usage: vm.sh ssh-run <name>}"
  shift
  exec ssh "${SSH_OPTS[@]}" -i "$KEY" -p "$RUN_PORT" "$SSH_USER@127.0.0.1" "$@"
}

cmd_stop_run() {
  local name="${1:?usage: vm.sh stop-run <name>}"
  ssh "${SSH_OPTS[@]}" -i "$KEY" -p "$RUN_PORT" "$SSH_USER@127.0.0.1" 'sudo poweroff' || true
  sleep 3
  rm -f "$(run_pidfile "$name")"
}

cmd_status() {
  echo "VM_DIR=$VM_DIR"
  ls -lh "$VM_DIR"/*.qcow2 "$BASELINE".sha256 "$SEED" 2>/dev/null || true
  if [[ -e "$VM_DIR/qemu.pid" ]] && kill -0 "$(cat "$VM_DIR/qemu.pid")" 2>/dev/null; then
    echo "provisioning VM: running (pid $(cat "$VM_DIR/qemu.pid"), ssh port $PROVISION_PORT)"
  else
    echo "provisioning VM: stopped"
  fi
  for pidfile in "$VM_DIR"/runs/*.pid; do
    [[ -e "$pidfile" ]] || continue
    local name
    name="$(basename "$pidfile" .pid)"
    if kill -0 "$(cat "$pidfile")" 2>/dev/null; then
      echo "run $name: running (pid $(cat "$pidfile"), ssh port $RUN_PORT)"
    fi
  done
}

case "${1:-}" in
  provision) cmd_provision ;;
  ssh) shift; cmd_ssh "$@" ;;
  stop) cmd_stop ;;
  snapshot) cmd_snapshot ;;
  run) shift; cmd_run "$@" ;;
  ssh-run) shift; cmd_ssh_run "$@" ;;
  stop-run) shift; cmd_stop_run "$@" ;;
  status) cmd_status ;;
  *) sed -n '2,20p' "$0"; exit 2 ;;
esac
