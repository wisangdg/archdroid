/**
 * Every streaming task the terminal modal can show, keyed by `taskType`.
 *
 * Starting a task, the consent dialog in front of it, and Retry all read this
 * one table, so a task cannot be startable but not retryable (Retry for
 * touch access once did nothing because a hand-written switch missed it).
 *
 * - `command`: the backend command.
 * - `title`: terminal title. `errorTitle`: toast when the command throws.
 * - `preview`: `preview_package_install` action; the dialog lists those
 *   packages and the same list is sent back as the approval.
 * - `confirm`: consent dialog shown first. Root tasks always have one, and
 *   Retry always goes back through it: the package plan may have changed.
 */
const ROOT_CONFIRM = {
  confirmText: "Proceed & Authorize Root",
  confirmVariant: "primary",
  iconType: "shield",
};

export const TASKS = {
  gapps: {
    command: "install_gapps",
    title: "Installing Google Play Services (GApps)",
    errorTitle: "GApps Error",
    preview: "gapps",
    confirm: {
      ...ROOT_CONFIRM,
      title: "Install Google Play Services (Root Required)",
      description:
        "This installer modifies the Waydroid Android system image (/var/lib/waydroid/overlay/system) to inject Google Play Store and Services framework.\n\n" +
        "• Source: waydroid_script (pinned commit 48dbfaf) • MindTheGapps / OpenGApps\n" +
        "• Privilege: Requires Administrator (Root) access via Polkit or sudo\n" +
        "• Consequence: Adds Google proprietary services to Android rootfs. Waydroid must be restarted after completion.\n\n" +
        "Do you want to proceed with root installation?",
    },
  },
  houdini: {
    command: "install_houdini",
    title: "Installing ARM Translation (libhoudini)",
    errorTitle: "ARM Translation Error",
    preview: "houdini",
    confirm: {
      ...ROOT_CONFIRM,
      title: "Install ARM Translation Layer (Root Required)",
      description:
        "This installer modifies the Waydroid Android system image (/var/lib/waydroid/overlay/system) and waydroid.cfg to inject Intel libhoudini ARM translation libraries.\n\n" +
        "• Source: waydroid_script (pinned commit 48dbfaf) • Intel proprietary prebuilts\n" +
        "• Privilege: Requires Administrator (Root) access via Polkit or sudo\n" +
        "• Consequence: Injects binary translation bridge (binfmt_misc) to execute ARM/ARM64 games on x86_64 CPUs.\n\n" +
        "Do you want to proceed with root installation?",
    },
  },
  multitouch: {
    command: "install_multitouch",
    title: "Declaring Touch Features for Google Play",
    errorTitle: "Touch Feature Error",
    confirm: {
      ...ROOT_CONFIRM,
      title: "Declare Touch Features (Root Required)",
      description:
        "This operation writes Android touchscreen hardware capability declarations to the Waydroid vendor overlay (/var/lib/waydroid/overlay).\n\n" +
        "• Source: ArchDroid internal hardware profile\n" +
        "• Privilege: Requires Administrator (Root) access via Polkit or sudo\n" +
        "• Consequence: Declares multitouch hardware features so Google Play Store permits downloading touch-dependent games. Requires restarting Waydroid.\n\n" +
        "Do you want to proceed with root installation?",
    },
  },
  "touch-access": {
    command: "install_touch_mode_access",
    title: "Granting Touch Mode Access",
    errorTitle: "Touch Mode Error",
    confirm: {
      ...ROOT_CONFIRM,
      title: "Enable Touch Mode Access (Root Required)",
      description:
        "This installs a udev rule at /etc/udev/rules.d/70-archdroid-touch.rules.\n\n" +
        "• Grants: the logged-in desktop user may read mice (never keyboards) and use /dev/uinput, which lets any program the user runs create virtual input devices, keyboards included (the same access Steam Input gets).\n" +
        "• Why: touch mode turns your mouse clicks over Android windows into finger touches.\n" +
        "• Privilege: Requires Administrator (Root) access via Polkit or sudo\n" +
        '• Undo: delete that file and run "udevadm control --reload-rules".\n\n' +
        "Do you want to proceed?",
    },
  },
  init: {
    command: "waydroid_init",
    title: "Initializing Waydroid System Image",
    errorTitle: "Init Error",
    preview: "init",
    confirm: {
      ...ROOT_CONFIRM,
      confirmText: "Initialize & Authorize Root",
      title: "Initialize Waydroid Container (Root Required)",
      description:
        "Initializing Waydroid will download the official LineageOS container images (~1.2 GB) and configure the LXC Android container under /var/lib/waydroid.\n\n" +
        "• Source: Official Waydroid / LineageOS system images\n" +
        "• Privilege: Requires Administrator (Root) access via Polkit or sudo\n" +
        "• Consequence: Sets up root container files and system partitions on disk.\n\n" +
        "Do you want to proceed?",
    },
  },
  wipe: {
    command: "waydroid_wipe",
    title: "Factory Reset (Wiping Waydroid Data)",
    errorTitle: "Wipe Failed",
    confirm: {
      title: "Factory Reset Waydroid?",
      description:
        "WARNING: This will completely destroy all Waydroid container data, installed games, GApps modules, and configurations. Back up anything you need first: Save APK (My Apps) keeps an app's installer, and Settings → Android Shared Storage lets you copy files out. App data cannot be saved. You will need to re-initialize the emulator from scratch. This cannot be undone!",
      confirmText: "Wipe Everything",
      confirmVariant: "danger",
      iconType: "danger",
    },
  },
  phantom: {
    command: "install_phantom",
    title: "Installing / Updating Phantom Keymapper",
    errorTitle: "Phantom Error",
  },
  logs: {
    command: "waydroid_log",
    title: "Waydroid Diagnostic Logs",
    errorTitle: "Log Read Error",
  },
};
