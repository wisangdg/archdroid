import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

const STATES = ["on", "off", "foreign"];

/** The card's state from a backend answer, or null for an error. */
const toDns = (res) =>
  res?.status === "success" && STATES.includes(res.data)
    ? { state: res.data, message: res.message ?? null }
    : null;

/**
 * AdGuard DNS in the Block Ads card: Android's Private DNS set to AdGuard
 * (src-tauri/src/privatedns.rs). `state` is `unknown` until Android has
 * booted and answered, then `on`, `off`, or `foreign`: a Private DNS server
 * ArchDroid did not set, which it leaves alone (`message` names it).
 */
export function useAdguardDns({ androidReady, addToast }) {
  const [dns, setDns] = useState({ state: "unknown", message: null });
  const [changing, setChanging] = useState(false);

  // Applies the stored choice too, so a fresh Android (Factory Reset) gets
  // AdGuard DNS again. adb answers only once Android has booted.
  useEffect(() => {
    if (!androidReady) return;
    invoke("private_dns_sync")
      .then((res) => {
        const next = toDns(res);
        if (next) setDns(next);
      })
      .catch((e) => console.error("Failed to sync AdGuard DNS:", e));
  }, [androidReady]);

  const setEnabled = useCallback(
    async (enabled) => {
      setChanging(true);
      try {
        const res = await invoke("private_dns_set", { enabled });
        const next = toDns(res);
        if (!next) {
          throw new Error(res?.message || "Android did not take the change.");
        }
        setDns(next);
        addToast({
          type: "info",
          title: "AdGuard DNS",
          message: {
            on: "Android now sends its DNS queries to AdGuard DNS (dns.adguard-dns.com).",
            off: "Android's Private DNS is back to what it was before.",
            foreign:
              "Android uses a Private DNS server ArchDroid did not set; it was left alone.",
          }[res.data],
          duration: 4000,
        });
      } catch (e) {
        addToast({
          type: "error",
          title: "AdGuard DNS",
          message: e?.message || String(e),
        });
      } finally {
        setChanging(false);
      }
    },
    [addToast]
  );

  return { ...dns, changing, setEnabled };
}
