<script setup lang="ts">
import type { useLongPress } from "../../composables/useLongPress";
import { useAppI18n } from "../../i18n";
import FavoriteIcon from "../FavoriteIcon.vue";
import ShortcutIcon from "../ShortcutIcon.vue";

defineProps<{
  showFavorite: boolean;
  favorited: boolean;
  favoriteLoading: boolean;
  favoritePending: boolean;
  favoriteActionLabel: string;
  favoriteButtonText: string;
  favoriteHandlers: ReturnType<typeof useLongPress>["handlers"];
  showDownload: boolean;
  canRead: boolean;
  canResume: boolean;
  downloadLoading: boolean;
  downloadPending: boolean;
  downloadButtonLabel: string;
  downloaded: boolean;
  resumeTitle: string;
}>();

const emit = defineEmits<{
  favorite: [];
  download: [];
  read: [];
  resume: [];
}>();

const { t } = useAppI18n();
</script>

<template>
  <div class="detail-topbar-actions">
    <span
      v-if="showFavorite"
      class="favorite-trigger"
      v-on="favoriteHandlers"
    >
      <NButton
        attr-type="button"
        :type="favorited ? 'primary' : 'default'"
        secondary
        :loading="favoriteLoading || favoritePending"
        :disabled="favoriteLoading || favoritePending"
        :aria-pressed="favorited"
        :aria-label="favoriteActionLabel"
        :aria-description="t('favorites.longPressHint')"
        aria-haspopup="dialog"
        aria-keyshortcuts="Shift+Enter"
        :title="favoriteActionLabel"
        class="favorite-button"
        @click="emit('favorite')"
      >
        <template #icon>
          <FavoriteIcon :filled="favorited" />
        </template>
        <span class="favorite-button__label">{{ favoriteButtonText }}</span>
      </NButton>
    </span>

    <NButton
      v-if="showDownload"
      attr-type="button"
      :type="downloaded ? 'primary' : 'default'"
      secondary
      :loading="downloadPending"
      :disabled="downloadLoading || downloadPending"
      :aria-label="downloadButtonLabel"
      :title="downloadButtonLabel"
      class="download-button"
      @click="emit('download')"
    >
      <template #icon>
        <ShortcutIcon name="downloads" />
      </template>
      <span class="download-button__label">{{ downloadButtonLabel }}</span>
    </NButton>

    <NButton
      v-if="canRead"
      attr-type="button"
      :type="canResume ? 'default' : 'primary'"
      :secondary="canResume"
      @click="emit('read')"
    >
      {{ t("detail.read") }}
    </NButton>

    <NButton
      v-if="canResume"
      attr-type="button"
      type="primary"
      :title="resumeTitle"
      @click="emit('resume')"
    >
      {{ t("detail.continueReading") }}
    </NButton>
  </div>
</template>

<style scoped>
.detail-topbar-actions {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 0.5rem;
  flex-wrap: wrap;
}

.favorite-trigger {
  display: inline-flex;
  touch-action: manipulation;
  user-select: none;
  -webkit-touch-callout: none;
}

.favorite-button__label,
.download-button__label {
  white-space: nowrap;
}

.download-button :deep(.shortcut-icon-svg) {
  width: 18px;
  height: 18px;
}

@media (max-width: 560px) {
  .favorite-button__label,
  .download-button__label {
    display: none;
  }

  /* With the label hidden, Naive UI's icon margin and horizontal padding make
     the icon look off-center in the still-wide button. Collapse it to a square
     icon target and hide the empty content flex item so only the icon (or
     loading spinner) remains centered. */
  .favorite-button,
  .download-button {
    width: 40px;
    min-width: 40px;
    height: 40px;
    padding: 0;
    justify-content: center;
  }

  .favorite-button :deep(.n-button__content),
  .download-button :deep(.n-button__content) {
    display: none;
  }

  .favorite-button :deep(.n-button__icon),
  .download-button :deep(.n-button__icon) {
    margin: 0;
  }
}
</style>
