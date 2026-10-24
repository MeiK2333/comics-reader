<script setup lang="ts">
import { computed, h } from "vue";
import type { DropdownOption } from "naive-ui";
import type { ComicPlatform } from "../api/comics";
import manhuaguiIcon from "../assets/platforms/manhuagui.svg";
import jmIcon from "../assets/platforms/jm.png";
import nhentaiIcon from "../assets/platforms/nhentai.svg";
import { useAppI18n } from "../i18n";
import type { MessageKey } from "../i18n";

interface PlatformOption {
  icon: string;
  nameKey: MessageKey;
  id: ComicPlatform;
}

const platforms: ReadonlyArray<PlatformOption> = [
  {
    icon: manhuaguiIcon,
    nameKey: "home.search.platforms.manhuagui",
    id: "manhuagui",
  },
  { icon: jmIcon, nameKey: "home.search.platforms.jm", id: "jm" },
  {
    icon: nhentaiIcon,
    nameKey: "home.search.platforms.nhentai",
    id: "nhentai",
  },
];

const props = defineProps<{
  platform: ComicPlatform;
}>();

const emit = defineEmits<{
  select: [platform: ComicPlatform];
}>();

const { t } = useAppI18n();

const platformOptions = computed<DropdownOption[]>(() =>
  platforms.map((option) => ({
    key: option.id,
    label: t(option.nameKey),
    icon: () => {
      const nhentai = option.id === "nhentai";
      return h("img", {
        src: option.icon,
        alt: "",
        style: {
          width: "20px",
          height: "20px",
          borderRadius: "4px",
          objectFit: "contain",
          boxSizing: "border-box",
          background: nhentai ? "#0c0c0c" : "transparent",
          padding: nhentai ? "3px 2px" : "0",
        },
      });
    },
  })),
);

const selectedPlatform = computed(
  () =>
    platforms.find((option) => option.id === props.platform) ?? platforms[0],
);

const triggerLabel = computed(
  () => `${t("home.search.platform")}: ${t(selectedPlatform.value.nameKey)}`,
);

function selectPlatform(key: string | number) {
  const next = platforms.find((option) => option.id === String(key));
  if (next && next.id !== props.platform) {
    emit("select", next.id);
  }
}
</script>

<template>
  <NDropdown
    trigger="click"
    :options="platformOptions"
    placement="bottom-start"
    @select="selectPlatform"
  >
    <NButton
      attr-type="button"
      quaternary
      circle
      size="large"
      class="platform-trigger"
      :aria-label="triggerLabel"
    >
      <template #icon>
        <img
          class="platform-trigger-icon"
          :class="{
            'platform-trigger-icon--nhentai': selectedPlatform.id === 'nhentai',
          }"
          :src="selectedPlatform.icon"
          alt=""
          draggable="false"
        />
      </template>
    </NButton>
  </NDropdown>
</template>

<style scoped>
.platform-trigger {
  flex: none;
}

.platform-trigger-icon {
  display: block;
  width: 30px;
  height: 30px;
  border-radius: 4px;
  object-fit: contain;
}

.platform-trigger-icon--nhentai {
  padding: 3px 2px;
  border-radius: 50%;
  background: #0c0c0c;
  box-sizing: border-box;
}

.platform-trigger :deep(.n-button__icon) {
  margin-right: 0;
}
</style>
