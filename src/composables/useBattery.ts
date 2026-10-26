import { computed, onBeforeUnmount, onMounted, ref } from "vue";

interface BatteryManagerLike extends EventTarget {
  readonly level: number;
  readonly charging: boolean;
}

interface NavigatorWithBattery extends Navigator {
  getBattery?: () => Promise<BatteryManagerLike>;
}

function isMobileWebView(): boolean {
  return /Android|iPhone|iPad|iPod/i.test(navigator.userAgent);
}

/**
 * Battery Status API access for the reader HUD.
 *
 * Desktop builds never show a battery, and not every WebView exposes the API.
 * Both cases resolve to `percent === null`, which the reader treats as "do not
 * render" rather than showing a misleading placeholder.
 */
export function useBattery() {
  const level = ref<number | null>(null);
  let manager: BatteryManagerLike | null = null;
  let disposed = false;

  function update() {
    if (!manager) {
      return;
    }
    const next = manager.level;
    level.value = Number.isFinite(next)
      ? Math.min(1, Math.max(0, next))
      : null;
  }

  function cleanup() {
    if (!manager) {
      return;
    }
    manager.removeEventListener("levelchange", update);
    manager.removeEventListener("chargingchange", update);
    manager = null;
  }

  onMounted(async () => {
    if (!isMobileWebView()) {
      return;
    }

    const getBattery = (navigator as NavigatorWithBattery).getBattery;
    if (typeof getBattery !== "function") {
      return;
    }

    try {
      const battery = await getBattery.call(navigator);
      if (disposed) {
        return;
      }
      manager = battery;
      update();
      battery.addEventListener("levelchange", update);
      battery.addEventListener("chargingchange", update);
    } catch {
      // Battery data is decorative; keep the reader usable without it.
    }
  });

  onBeforeUnmount(() => {
    disposed = true;
    cleanup();
  });

  const percent = computed(() =>
    level.value === null ? null : Math.round(level.value * 100),
  );

  return { percent };
}
