<script setup lang="ts">
import { computed, h } from "vue";
import type { DropdownOption } from "naive-ui";
import type { ComicPlatform } from "../platforms";
import { platformLabel, platformList } from "../platforms";
import { useAppI18n } from "../i18n";

const props = defineProps<{
  platform: ComicPlatform;
}>();

const emit = defineEmits<{
  select: [platform: ComicPlatform];
}>();

const { t } = useAppI18n();

const platformOptions = computed<DropdownOption[]>(() =>
  platformList.map((option) => ({
    key: option.id,
    label: platformLabel(option.id),
    icon: () => {
      const chip = option.iconStyle === "dark-chip";
      return h("img", {
        src: option.icon,
        alt: "",
        style: {
          width: "20px",
          height: "20px",
          borderRadius: "4px",
          objectFit: "contain",
          boxSizing: "border-box",
          background: chip ? "#0c0c0c" : "transparent",
          padding: chip ? "3px 2px" : "0",
        },
      });
    },
  })),
);

const selectedPlatform = computed(
  () =>
    platformList.find((option) => option.id === props.platform) ??
    platformList[0],
);

const triggerLabel = computed(
  () => `${t("home.search.platform")}: ${platformLabel(props.platform)}`,
);

function selectPlatform(key: string | number) {
  const next = platformList.find((option) => option.id === String(key));
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
            'platform-trigger-icon--dark-chip':
              selectedPlatform.iconStyle === 'dark-chip',
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

.platform-trigger-icon--dark-chip {
  padding: 3px 2px;
  border-radius: 50%;
  background: #0c0c0c;
  box-sizing: border-box;
}

.platform-trigger :deep(.n-button__icon) {
  margin-right: 0;
}
</style>