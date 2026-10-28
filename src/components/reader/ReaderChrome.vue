<script setup lang="ts">
import { computed } from "vue";
import { useBattery } from "../../composables/useBattery";
import { isMobileWebView } from "../../device";
import { useAppI18n } from "../../i18n";
import ReaderBattery from "./ReaderBattery.vue";
import ReaderPageJump from "./ReaderPageJump.vue";

defineProps<{
  chromeVisible: boolean;
  title: string;
  chapterTitle: string;
  counterPage: number;
  total: number;
  canPrevious: boolean;
  canNext: boolean;
  hasPreviousChapter: boolean;
  hasNextChapter: boolean;
  trackAnimating: boolean;
  boundaryLeaving: boolean;
  isFullscreen: boolean;
  percent: number;
}>();

const emit = defineEmits<{
  close: [];
  goToPage: [page: number];
  previousPage: [];
  nextPage: [];
  previousChapter: [];
  nextChapter: [];
  zoomIn: [];
  zoomOut: [];
  resetZoom: [];
  toggleFullscreen: [];
}>();

const { t } = useAppI18n();
const { percent: batteryPercent } = useBattery();
const showFullscreen = !isMobileWebView();
const batteryLabel = computed(() =>
  batteryPercent.value === null
    ? ""
    : t("reader.batteryLevel", { level: batteryPercent.value }),
);
</script>

<template>
  <header
    class="viewer__bar viewer__bar--top"
    :class="{ 'viewer__bar--hidden': !chromeVisible }"
    :aria-hidden="!chromeVisible"
    :inert="!chromeVisible || undefined"
  >
    <NButton
      attr-type="button"
      quaternary
      circle
      size="large"
      :aria-label="t('reader.back')"
      :title="t('reader.back')"
      @click="emit('close')"
    >
      <template #icon>
        <svg
          class="viewer__icon"
          viewBox="0 0 24 24"
          aria-hidden="true"
          focusable="false"
        >
          <path
            d="M15 5l-7 7 7 7"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
          />
        </svg>
      </template>
    </NButton>

    <div class="viewer__heading">
      <span class="viewer__title">{{ title }}</span>
      <span v-if="chapterTitle" class="viewer__chapter">
        {{ chapterTitle }}
      </span>
    </div>

    <ReaderBattery
      v-if="batteryPercent !== null"
      :level="batteryPercent"
      :label="batteryLabel"
    />

    <span class="viewer__counter">
      <ReaderPageJump
        :page="counterPage"
        :total="total"
        :disabled="trackAnimating || boundaryLeaving"
        @submit="emit('goToPage', $event)"
      />
    </span>
  </header>

  <footer
    class="viewer__bar viewer__bar--bottom"
    :class="{ 'viewer__bar--hidden': !chromeVisible }"
    :aria-hidden="!chromeVisible"
    :inert="!chromeVisible || undefined"
  >
    <div class="viewer__nav">
      <NButton
        attr-type="button"
        quaternary
        circle
        :disabled="!canPrevious"
        :aria-label="t('reader.previousPage')"
        :title="t('reader.previousPage')"
        @click="emit('previousPage')"
      >
        <template #icon>
          <svg
            class="viewer__icon"
            viewBox="0 0 24 24"
            aria-hidden="true"
            focusable="false"
          >
            <path
              d="M15 5l-7 7 7 7"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
              stroke-linecap="round"
              stroke-linejoin="round"
            />
          </svg>
        </template>
      </NButton>

      <span class="viewer__counter viewer__counter--bottom">
        <ReaderPageJump
          :page="counterPage"
          :total="total"
          :disabled="trackAnimating || boundaryLeaving"
          @submit="emit('goToPage', $event)"
        />
      </span>

      <NButton
        attr-type="button"
        quaternary
        circle
        :disabled="!canNext"
        :aria-label="t('reader.nextPage')"
        :title="t('reader.nextPage')"
        @click="emit('nextPage')"
      >
        <template #icon>
          <svg
            class="viewer__icon"
            viewBox="0 0 24 24"
            aria-hidden="true"
            focusable="false"
          >
            <path
              d="M9 5l7 7-7 7"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
              stroke-linecap="round"
              stroke-linejoin="round"
            />
          </svg>
        </template>
      </NButton>

      <NButton
        v-if="hasPreviousChapter"
        attr-type="button"
        quaternary
        size="small"
        @click="emit('previousChapter')"
      >
        {{ t("reader.previousChapter") }}
      </NButton>
      <NButton
        v-if="hasNextChapter"
        attr-type="button"
        quaternary
        size="small"
        @click="emit('nextChapter')"
      >
        {{ t("reader.nextChapter") }}
      </NButton>
    </div>

    <div class="viewer__tools">
      <NButton
        attr-type="button"
        quaternary
        circle
        :disabled="trackAnimating"
        :aria-label="t('reader.zoomOut')"
        :title="t('reader.zoomOut')"
        @click="emit('zoomOut')"
      >
        <template #icon>
          <span aria-hidden="true">−</span>
        </template>
      </NButton>
      <span class="viewer__zoom">{{ percent }}%</span>
      <NButton
        attr-type="button"
        quaternary
        circle
        :disabled="trackAnimating"
        :aria-label="t('reader.zoomIn')"
        :title="t('reader.zoomIn')"
        @click="emit('zoomIn')"
      >
        <template #icon>
          <span aria-hidden="true">+</span>
        </template>
      </NButton>
      <NButton
        attr-type="button"
        quaternary
        size="small"
        :disabled="trackAnimating"
        :aria-label="t('reader.resetZoom')"
        :title="t('reader.resetZoom')"
        @click="emit('resetZoom')"
      >
        1:1
      </NButton>
      <NButton
        v-if="showFullscreen"
        attr-type="button"
        quaternary
        circle
        :aria-label="
          isFullscreen
            ? t('reader.exitFullscreen')
            : t('reader.fullscreen')
        "
        :title="
          isFullscreen
            ? t('reader.exitFullscreen')
            : t('reader.fullscreen')
        "
        @click="emit('toggleFullscreen')"
      >
        <template #icon>
          <svg
            class="viewer__icon"
            viewBox="0 0 24 24"
            aria-hidden="true"
            focusable="false"
          >
            <path
              d="M8 3H3v5M16 3h5v5M8 21H3v-5M16 21h5v-5"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
              stroke-linecap="round"
              stroke-linejoin="round"
            />
          </svg>
        </template>
      </NButton>
    </div>
  </footer>

  <div v-show="!chromeVisible" class="viewer__hud">
    <ReaderBattery
      v-if="batteryPercent !== null"
      :level="batteryPercent"
      :label="batteryLabel"
    />
    <span class="viewer__hud-page viewer__hud-jump">
      <ReaderPageJump
        :page="counterPage"
        :total="total"
        :disabled="trackAnimating || boundaryLeaving"
        @submit="emit('goToPage', $event)"
      />
    </span>
  </div>

  <p class="sr-only" aria-live="polite">
    {{
      t("reader.pageAnnouncement", {
        current: counterPage,
        total,
      })
    }}
  </p>
</template>

<style scoped>
.viewer__bar {
  position: absolute;
  right: 0;
  left: 0;
  z-index: 3;
  display: flex;
  align-items: center;
  gap: 0.75rem;
  padding: 0.4rem 0.75rem;
  background: var(--app-background);
  transition:
    opacity 180ms ease,
    transform 180ms ease;
}

.viewer__bar--top {
  top: 0;
  min-height: 56px;
  border-bottom: 1px solid rgba(127, 127, 127, 0.12);
}

.viewer__bar--bottom {
  bottom: 0;
  min-height: 60px;
  padding-bottom: calc(0.4rem + env(safe-area-inset-bottom, 0px));
  border-top: 1px solid rgba(127, 127, 127, 0.12);
}

.viewer__bar--hidden {
  opacity: 0;
  pointer-events: none;
}

.viewer__bar--top.viewer__bar--hidden {
  transform: translateY(-100%);
}

.viewer__bar--bottom.viewer__bar--hidden {
  transform: translateY(100%);
}

.viewer__icon {
  display: block;
  width: 20px;
  height: 20px;
}

.viewer__heading {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-width: 0;
  line-height: 1.3;
}

.viewer__title,
.viewer__chapter {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.viewer__title {
  font-size: 0.95rem;
  font-weight: 600;
}

.viewer__chapter {
  font-size: 0.78rem;
  opacity: 0.65;
}

.viewer__counter {
  display: inline-flex;
  flex: none;
  align-items: center;
  font-size: 0.85rem;
  font-variant-numeric: tabular-nums;
}

.viewer__hud {
  position: absolute;
  top: 0.5rem;
  right: calc(0.75rem + var(--app-safe-area-right, 0px));
  z-index: 2;
  display: flex;
  align-items: center;
  gap: 0.5rem;
  padding: 0.3rem 0.6rem;
  border: 1px solid rgba(127, 127, 127, 0.18);
  border-radius: 999px;
  background: var(--app-background);
  background: color-mix(in srgb, var(--app-background) 82%, transparent);
  box-shadow: 0 2px 10px rgba(0, 0, 0, 0.12);
  backdrop-filter: blur(8px);
  font-size: 0.82rem;
  font-variant-numeric: tabular-nums;
  pointer-events: none;
}

.viewer__hud-jump {
  pointer-events: auto;
}

.viewer__nav,
.viewer__tools {
  display: flex;
  align-items: center;
  gap: 0.3rem;
}

.viewer__nav {
  flex: 1;
  justify-content: flex-start;
}

.viewer__tools {
  flex: none;
  justify-content: flex-end;
}

.viewer__counter--bottom {
  justify-content: center;
  min-width: 4.5rem;
  text-align: center;
}

.viewer__zoom {
  min-width: 3.1rem;
  text-align: center;
  font-size: 0.85rem;
  font-variant-numeric: tabular-nums;
  opacity: 0.7;
}

.sr-only {
  position: absolute;
  width: 1px;
  height: 1px;
  padding: 0;
  margin: -1px;
  overflow: hidden;
  clip: rect(0, 0, 0, 0);
  white-space: nowrap;
  border: 0;
}

@media (max-width: 620px) {
  .viewer__bar {
    padding-inline: 0.45rem;
  }

  .viewer__title {
    font-size: 0.88rem;
  }

  .viewer__chapter {
    font-size: 0.72rem;
  }

  .viewer__counter {
    font-size: 0.78rem;
  }

  .viewer__tools {
    gap: 0.1rem;
  }

  .viewer__zoom {
    min-width: 2.6rem;
  }
}

@media (max-width: 430px) {
  .viewer__bar--bottom {
    flex-wrap: wrap;
    row-gap: 0.15rem;
  }

  .viewer__nav,
  .viewer__tools {
    flex: 1 1 100%;
    justify-content: center;
  }
}
</style>
