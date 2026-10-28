<script setup lang="ts">
import {
  nextTick,
  onBeforeUnmount,
  onMounted,
  ref,
  watch,
} from "vue";
import { DEFAULT_GROUP_ID, type FavoriteGroup } from "../../api/favorites";
import { useAppI18n } from "../../i18n";

const props = defineProps<{
  groups: FavoriteGroup[];
  /** Number of works across every group, shown on the "all" chip. */
  total: number;
  selectedGroupId: string | null;
  loading: boolean;
}>();

const emit = defineEmits<{
  select: [groupId: string | null];
  create: [];
  manage: [];
}>();

const { t } = useAppI18n();

const barEl = ref<HTMLElement | null>(null);
const viewportEl = ref<HTMLElement | null>(null);
const stuck = ref(false);
const atStart = ref(true);
const atEnd = ref(true);

let stickyTop = 0;
let frame = 0;
let resizeObserver: ResizeObserver | null = null;

function groupLabel(group: FavoriteGroup): string {
  return group.id === DEFAULT_GROUP_ID
    ? t("favorites.defaultGroup")
    : group.name;
}

/** Fade only the edge that hides content, so a short strip looks untouched. */
function updateEdges() {
  const el = viewportEl.value;
  if (!el) {
    return;
  }

  const max = el.scrollWidth - el.clientWidth;
  atStart.value = el.scrollLeft <= 1;
  atEnd.value = max <= 1 || el.scrollLeft >= max - 1;
}

function measure() {
  const el = barEl.value;
  if (!el) {
    return;
  }

  stickyTop = Number.parseFloat(getComputedStyle(el).top) || 0;
  updateEdges();
}

function updateStuck() {
  frame = 0;
  const el = barEl.value;
  if (!el) {
    return;
  }
  stuck.value = el.getBoundingClientRect().top <= stickyTop + 0.5;
}

function onScroll() {
  if (frame !== 0) {
    return;
  }
  frame = requestAnimationFrame(updateStuck);
}

function scrollSelectedIntoView() {
  const viewport = viewportEl.value;
  if (!viewport || typeof viewport.scrollTo !== "function") {
    return;
  }

  const selected = viewport.querySelector<HTMLElement>(
    "[data-selected='true']",
  );
  if (!selected) {
    return;
  }

  const centered =
    selected.offsetLeft - (viewport.clientWidth - selected.offsetWidth) / 2;
  viewport.scrollTo({ left: Math.max(0, centered), behavior: "smooth" });
}

onMounted(() => {
  measure();
  updateStuck();
  window.addEventListener("scroll", onScroll, { passive: true });
  window.addEventListener("resize", measure, { passive: true });

  if (typeof ResizeObserver !== "undefined" && viewportEl.value) {
    resizeObserver = new ResizeObserver(updateEdges);
    resizeObserver.observe(viewportEl.value);
  }

  void nextTick(updateEdges);
});

onBeforeUnmount(() => {
  window.removeEventListener("scroll", onScroll);
  window.removeEventListener("resize", measure);
  resizeObserver?.disconnect();
  if (frame !== 0) {
    cancelAnimationFrame(frame);
  }
});

watch(
  () => props.selectedGroupId,
  () => {
    void nextTick(scrollSelectedIntoView);
  },
);

watch(
  () => props.groups.length,
  () => {
    void nextTick(updateEdges);
  },
);
</script>

<template>
  <div ref="barEl" class="group-bar" :class="{ 'group-bar--stuck': stuck }">
    <div
      ref="viewportEl"
      class="group-bar__viewport"
      :class="{
        'group-bar__viewport--fade-start': !atStart,
        'group-bar__viewport--fade-end': !atEnd,
      }"
      role="group"
      :aria-label="t('favorites.groupsLabel')"
      @scroll.passive="updateEdges"
    >
      <NButton
        size="small"
        secondary
        class="group-bar__chip"
        :type="selectedGroupId === null ? 'primary' : 'default'"
        :aria-pressed="selectedGroupId === null"
        :disabled="loading"
        :data-selected="selectedGroupId === null"
        @click="emit('select', null)"
      >
        <span class="group-bar__chip-label">{{ t("favorites.allGroups") }}</span>
        <span class="group-bar__chip-count">{{ total }}</span>
      </NButton>

      <NButton
        v-for="group in groups"
        :key="group.id"
        size="small"
        secondary
        class="group-bar__chip"
        :type="selectedGroupId === group.id ? 'primary' : 'default'"
        :aria-pressed="selectedGroupId === group.id"
        :disabled="loading"
        :data-selected="selectedGroupId === group.id"
        @click="emit('select', group.id)"
      >
        <span class="group-bar__chip-label">{{ groupLabel(group) }}</span>
        <span class="group-bar__chip-count">{{ group.count }}</span>
      </NButton>
    </div>

    <!-- Pinned outside the scroll container: these must never be scrolled out
         of reach on a narrow screen. -->
    <div class="group-bar__actions">
      <NButton
        attr-type="button"
        size="small"
        quaternary
        class="group-bar__action"
        :aria-label="t('favorites.group.create')"
        :title="t('favorites.group.create')"
        :disabled="loading"
        @click="emit('create')"
      >
        <template #icon>
          <svg
            class="group-bar__action-icon"
            viewBox="0 0 24 24"
            aria-hidden="true"
            focusable="false"
          >
            <path
              d="M12 5v14M5 12h14"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
              stroke-linecap="round"
            />
          </svg>
        </template>
        <span class="group-bar__action-label">
          {{ t("favorites.group.create") }}
        </span>
      </NButton>

      <NButton
        attr-type="button"
        size="small"
        quaternary
        class="group-bar__action"
        :aria-label="t('favorites.group.manage')"
        :title="t('favorites.group.manage')"
        :disabled="loading"
        @click="emit('manage')"
      >
        <template #icon>
          <svg
            class="group-bar__action-icon"
            viewBox="0 0 24 24"
            aria-hidden="true"
            focusable="false"
          >
            <path
              d="M4 7h9M19 7h1M4 17h3M13 17h7"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
              stroke-linecap="round"
            />
            <circle
              cx="16"
              cy="7"
              r="2.2"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
            />
            <circle
              cx="10"
              cy="17"
              r="2.2"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
            />
          </svg>
        </template>
        <span class="group-bar__action-label">
          {{ t("favorites.group.manage") }}
        </span>
      </NButton>
    </div>
  </div>
</template>

<style scoped>
.group-bar {
  position: sticky;
  top: var(--app-safe-area-top);
  /* Below App.vue's sticky safe-area strip (z-index 10). */
  z-index: 9;
  display: flex;
  align-items: center;
  gap: 0.4rem;
  padding: 0.4rem 0;
  background-color: var(--app-background);
  transition: box-shadow 160ms ease;
}

.group-bar--stuck {
  box-shadow: 0 8px 12px -10px rgba(0, 0, 0, 0.45);
}

.group-bar__viewport {
  flex: 1;
  min-width: 0;
  display: flex;
  gap: 0.35rem;
  overflow-x: auto;
  /* Do not hand horizontal swipes to the browser's back gesture. */
  overscroll-behavior-x: contain;
  scroll-snap-type: x proximity;
  scroll-padding-inline: 0.25rem;
  scrollbar-width: none;
}

.group-bar__viewport::-webkit-scrollbar {
  display: none;
}

.group-bar__viewport--fade-start {
  -webkit-mask-image: linear-gradient(to right, transparent, #000 16px);
  mask-image: linear-gradient(to right, transparent, #000 16px);
}

.group-bar__viewport--fade-end {
  -webkit-mask-image: linear-gradient(to left, transparent, #000 16px);
  mask-image: linear-gradient(to left, transparent, #000 16px);
}

.group-bar__viewport--fade-start.group-bar__viewport--fade-end {
  -webkit-mask-image: linear-gradient(
    to right,
    transparent,
    #000 16px,
    #000 calc(100% - 16px),
    transparent
  );
  mask-image: linear-gradient(
    to right,
    transparent,
    #000 16px,
    #000 calc(100% - 16px),
    transparent
  );
}

.group-bar__chip {
  flex: none;
  scroll-snap-align: start;
}

.group-bar__chip-label {
  display: inline-block;
  max-width: 10em;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  vertical-align: bottom;
}

.group-bar__chip-count {
  margin-left: 0.3rem;
  font-size: 0.72rem;
  opacity: 0.6;
  font-variant-numeric: tabular-nums;
}

.group-bar__actions {
  display: flex;
  flex: none;
  gap: 0.1rem;
}

.group-bar__action-icon {
  display: block;
  width: 18px;
  height: 18px;
}

@media (max-width: 560px) {
  .group-bar__viewport :deep(.n-button) {
    min-height: 40px;
  }

  /* Icon-only on phones: the strip and the pinned actions have to share a
     360px line without the labels eating it. */
  .group-bar__action {
    width: 40px;
    min-width: 40px;
    height: 40px;
    padding: 0;
  }

  .group-bar__action :deep(.n-button__content) {
    display: none;
  }

  .group-bar__action :deep(.n-button__icon) {
    margin: 0;
  }
}
</style>
