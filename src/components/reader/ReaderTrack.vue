<script setup lang="ts">
import { computed } from "vue";
import type { CSSProperties } from "vue";
import type { ComicPlatform } from "../../api/comics";
import type { ReaderPage } from "../../api/reader";
import type { ReaderImageSize } from "../../composables/useReaderZoom";
import type { ReaderDirection } from "../../stores/settings";
import ReaderPageImage from "./ReaderPageImage.vue";

const props = defineProps<{
  pages: ReaderPage[];
  platform: ComicPlatform;
  direction: ReaderDirection;
  displayPage: number;
  total: number;
  trackStyle: CSSProperties;
  imageStyle: CSSProperties;
  identityStyle: CSSProperties;
}>();

const emit = defineEmits<{
  size: [number: number, size: ReaderImageSize];
}>();

interface PageSlot {
  page: ReaderPage;
  number: number;
}

const pageSlots = computed<(PageSlot | null)[]>(() => {
  const slots: (PageSlot | null)[] = [];
  for (
    let number = props.displayPage - 1;
    number <= props.displayPage + 1;
    number += 1
  ) {
    const page = props.pages[number - 1];
    slots.push(page ? { page, number } : null);
  }
  return slots;
});
</script>

<template>
  <div
    class="viewer__track"
    :class="`viewer__track--${direction}`"
    :style="trackStyle"
  >
    <div
      v-for="(slot, index) in pageSlots"
      :key="
        slot
          ? `${slot.number}:${slot.page.localPath ?? slot.page.url}`
          : `empty:${index}`
      "
      class="viewer__slot"
      :class="{ 'viewer__slot--empty': slot === null }"
      :aria-hidden="slot?.number !== displayPage"
    >
      <ReaderPageImage
        v-if="slot"
        :platform="platform"
        :page="slot.page"
        :page-number="slot.number"
        :total="total"
        :image-style="
          slot.number === displayPage ? imageStyle : identityStyle
        "
        @size="emit('size', slot.number, $event)"
      />
    </div>
  </div>
</template>

<style scoped>
.viewer__track {
  position: absolute;
  top: 0;
  left: 0;
  display: flex;
  will-change: transform;
}

.viewer__track--ltr,
.viewer__track--rtl {
  width: 300%;
  height: 100%;
  flex-direction: row;
}

.viewer__track--rtl {
  flex-direction: row-reverse;
}

.viewer__track--ttb {
  width: 100%;
  height: 300%;
  flex-direction: column;
}

.viewer__slot {
  position: relative;
  flex: 0 0 33.333333%;
  overflow: hidden;
}

.viewer__track--ltr .viewer__slot,
.viewer__track--rtl .viewer__slot {
  width: 33.333333%;
  height: 100%;
}

.viewer__track--ttb .viewer__slot {
  width: 100%;
  height: 33.333333%;
}

.viewer__slot--empty {
  background: var(--app-background);
}
</style>
