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
 *   `description` is one sentence; `details` are the labelled rows under it
 *   (what changes, where it comes from, which access, how to undo it).
 */
const ROOT_CONFIRM = {
  confirmText: "Proceed & Authorize Root",
  confirmVariant: "primary",
  iconType: "shield",
};

const ROOT_ACCESS = {
  label: "Access",
  text: "Administrator rights through polkit or sudo, for this action only.",
};

const UNDO_BY_RESET = {
  label: "Undo",
  text: "No separate uninstall; Factory Reset (Settings) removes it together with all Android data.",
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
        "Adds Google Play Store and Google Play Services to the Android system image.",
      details: [
        {
          label: "Changes",
          text: "Files in /var/lib/waydroid/overlay/system. Restart Waydroid afterwards.",
        },
        {
          label: "Source",
          text: "waydroid_script (pinned commit 48dbfaf), which downloads MindTheGapps or OpenGApps.",
        },
        ROOT_ACCESS,
        UNDO_BY_RESET,
      ],
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
        "Adds Intel libhoudini so apps built only for ARM can run on an x86_64 CPU.",
      details: [
        {
          label: "Changes",
          text: "Files in /var/lib/waydroid/overlay/system and settings in waydroid.cfg.",
        },
        {
          label: "Source",
          text: "waydroid_script (pinned commit 48dbfaf), with Intel's prebuilt libraries (proprietary).",
        },
        ROOT_ACCESS,
        UNDO_BY_RESET,
      ],
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
        "Makes Android declare a multitouch screen, so Google Play offers games that require one.",
      details: [
        {
          label: "Changes",
          text: "Writes one file: /var/lib/waydroid/overlay/vendor/etc/permissions/android.hardware.touchscreen.multitouch.xml. Restart Waydroid afterwards.",
        },
        {
          label: "Source",
          text: "Built into ArchDroid; nothing is downloaded.",
        },
        ROOT_ACCESS,
        {
          label: "Undo",
          text: "Delete that file as root and restart Waydroid.",
        },
      ],
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
        "Lets touch mode turn your mouse clicks over Android windows into finger touches.",
      details: [
        {
          label: "Changes",
          text: "Installs the udev rule /etc/udev/rules.d/70-archdroid-touch.rules.",
        },
        {
          label: "Grants",
          text: "The logged-in desktop user may read mice (never keyboards) and use /dev/uinput, which lets any program the user runs create virtual input devices, keyboards included (the same access Steam Input gets).",
        },
        ROOT_ACCESS,
        {
          label: "Undo",
          text: 'Delete that file and run "udevadm control --reload-rules".',
        },
      ],
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
        "Downloads the Android system images (about 1.2 GB) and sets up the Waydroid container.",
      details: [
        {
          label: "Changes",
          text: "Creates the container and its system partitions under /var/lib/waydroid.",
        },
        {
          label: "Source",
          text: "The official Waydroid LineageOS images.",
        },
        ROOT_ACCESS,
        {
          label: "Undo",
          text: "Factory Reset (Settings) deletes everything this creates.",
        },
      ],
    },
  },
  wipe: {
    command: "waydroid_wipe",
    title: "Factory Reset (Wiping Waydroid Data)",
    errorTitle: "Wipe Failed",
    confirm: {
      title: "Factory Reset Waydroid?",
      description:
        "Deletes the whole Waydroid container: installed apps and their data, add-ons such as GApps, and settings.",
      details: [
        {
          label: "Deletes",
          text: "/var/lib/waydroid, ~/.local/share/waydroid and Waydroid's launcher entries.",
        },
        {
          label: "Save first",
          text: "Save APK (My Apps) keeps an app's installer; Settings → Android Shared Storage lets you copy files out. App data cannot be saved.",
        },
        {
          label: "Afterwards",
          text: "Initialize the emulator again from scratch.",
        },
        { label: "Undo", text: "Not possible." },
      ],
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
