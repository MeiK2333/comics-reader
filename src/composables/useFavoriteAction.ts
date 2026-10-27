import { computed, ref, watch } from "vue";
import type { ComputedRef, Ref } from "vue";
import { useMessage } from "naive-ui";
import type { ComicPlatform } from "../api/comics";
import {
  addFavorite,
  getFavorite,
  removeFavorite,
  type FavoriteEntry,
} from "../api/favorites";
import { useErrorMessage } from "./useErrorMessage";
import { useLatestRequest } from "./useLatestRequest";
import { useLongPress } from "./useLongPress";
import { useAppI18n } from "../i18n";

/**
 * The minimal snapshot the favorite endpoints need. Both the online detail
 * and the downloaded detail can supply it without pretending to be the other
 * detail payload.
 */
export interface FavoriteSnapshot {
  platform: ComicPlatform;
  title: string;
  thumb: string | null;
}

export interface UseFavoriteActionOptions {
  platform: ComputedRef<ComicPlatform | null>;
  id: ComputedRef<string>;
  detail: Ref<FavoriteSnapshot | null>;
}

/**
 * Favorite state and the short-press / long-press behavior of the detail
 * button. The group picker owns its own group list; this composable only opens
 * it and stores the entry returned by `saved`.
 */
export function useFavoriteAction(options: UseFavoriteActionOptions) {
  const message = useMessage();
  const { t } = useAppI18n();
  const formatError = useErrorMessage();
  const latest = useLatestRequest();

  const entry = ref<FavoriteEntry | null>(null);
  const loading = ref(false);
  const pending = ref(false);
  const showGroupPicker = ref(false);

  const favorited = computed(() => entry.value !== null);
  const currentGroupId = computed(() => entry.value?.groupId ?? null);
  const favoriteButtonText = computed(() =>
    favorited.value ? t("favorites.added") : t("favorites.add"),
  );
  const favoriteActionLabel = computed(() =>
    favorited.value ? t("favorites.remove") : t("favorites.addTitle"),
  );

  const longPress = useLongPress({
    disabled: () => !options.detail.value || loading.value || pending.value,
    onLongPress: () => {
      if (options.detail.value) {
        showGroupPicker.value = true;
      }
    },
  });

  function reset() {
    latest.invalidate();
    entry.value = null;
    loading.value = false;
    pending.value = false;
    showGroupPicker.value = false;
  }

  async function load() {
    const current = options.detail.value;
    const comicPlatform = options.platform.value;
    const comicId = options.id.value;
    if (!current || !comicPlatform || !comicId) {
      return;
    }

    const token = latest.begin();
    loading.value = true;

    try {
      const saved = await getFavorite(comicPlatform, comicId);
      if (latest.isCurrent(token)) {
        entry.value = saved;
      }
    } catch {
      // The status lookup is auxiliary. The toggle is idempotent, so a failure
      // should leave the button usable instead of blocking the detail page.
      if (latest.isCurrent(token)) {
        entry.value = null;
      }
    } finally {
      if (latest.isCurrent(token)) {
        loading.value = false;
      }
    }
  }

  watch(options.detail, (current) => {
    if (!current) {
      reset();
      return;
    }
    showGroupPicker.value = false;
    void load();
  });

  function handleClick() {
    if (longPress.consumeClick()) {
      return;
    }
    void toggle();
  }

  async function toggle() {
    const current = options.detail.value;
    const comicPlatform = options.platform.value;
    const comicId = options.id.value;
    if (
      !current ||
      !comicPlatform ||
      !comicId ||
      loading.value ||
      pending.value
    ) {
      return;
    }

    const token = latest.begin();
    pending.value = true;

    try {
      if (favorited.value) {
        await removeFavorite(comicPlatform, comicId);
        if (latest.isCurrent(token)) {
          entry.value = null;
          message.success(t("favorites.removedToast"));
        }
      } else {
        const saved = await addFavorite({
          platform: current.platform,
          id: comicId,
          title: current.title,
          thumb: current.thumb,
        });
        if (latest.isCurrent(token)) {
          entry.value = saved;
          message.success(t("favorites.addedToast"));
        }
      }
    } catch (cause) {
      if (latest.isCurrent(token)) {
        message.error(formatError(cause));
      }
    } finally {
      if (latest.isCurrent(token)) {
        pending.value = false;
      }
    }
  }

  function onSaved(saved: FavoriteEntry) {
    entry.value = saved;
    showGroupPicker.value = false;
  }

  return {
    entry,
    favorited,
    currentGroupId,
    loading,
    pending,
    showGroupPicker,
    favoriteButtonText,
    favoriteActionLabel,
    handlers: longPress.handlers,
    handleClick,
    toggle,
    onSaved,
    reset,
  };
}
