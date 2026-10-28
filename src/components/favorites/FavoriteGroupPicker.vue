<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useMessage } from "naive-ui";
import type { ComicPlatform } from "../../platforms";
import {
  DEFAULT_GROUP_ID,
  addFavorite,
  createFavoriteGroup,
  listFavoriteGroups,
  type FavoriteEntry,
  type FavoriteGroup,
} from "../../api/favorites";
import { useErrorMessage } from "../../composables/useErrorMessage";
import { toAppError, type AppError } from "../../errors";
import { useAppI18n } from "../../i18n";
import AppSheet from "../AppSheet.vue";
import {
  GROUP_NAME_ISSUE_KEYS,
  GROUP_NAME_MAX_LENGTH,
  groupNameIssue,
} from "./favorite-group-name";

const props = defineProps<{
  show: boolean;
  platform: ComicPlatform;
  id: string;
  title: string;
  thumb: string;
  favorited: boolean;
  currentGroupId: string | null;
}>();

const emit = defineEmits<{
  "update:show": [value: boolean];
  saved: [entry: FavoriteEntry];
}>();

const { t } = useAppI18n();
const message = useMessage();
const formatError = useErrorMessage();

const groups = ref<FavoriteGroup[]>([]);
const loading = ref(false);
const loadError = ref<AppError | null>(null);
const selectedGroupId = ref(DEFAULT_GROUP_ID);
const newGroupName = ref("");
const nameError = ref("");
const creating = ref(false);
const saving = ref(false);
let groupsRequestId = 0;

const pickerTitle = computed(() =>
  props.favorited ? t("favorites.moveToGroup") : t("favorites.chooseGroup"),
);
const loadErrorText = computed(() =>
  loadError.value ? formatError(loadError.value) : "",
);
const selectedGroup = computed(() =>
  groups.value.find((group) => group.id === selectedGroupId.value),
);
const canConfirm = computed(
  () =>
    !loading.value &&
    !loadError.value &&
    !saving.value &&
    selectedGroup.value !== undefined,
);

function groupLabel(group: FavoriteGroup): string {
  return group.id === DEFAULT_GROUP_ID
    ? t("favorites.defaultGroup")
    : group.name;
}

async function loadGroups() {
  const current = ++groupsRequestId;
  loading.value = true;
  loadError.value = null;

  try {
    const result = await listFavoriteGroups();
    if (current !== groupsRequestId) {
      return;
    }
    groups.value = result.groups;
    const desired = props.currentGroupId ?? DEFAULT_GROUP_ID;
    selectedGroupId.value = result.groups.some(
      (group) => group.id === desired,
    )
      ? desired
      : DEFAULT_GROUP_ID;
  } catch (cause) {
    if (current !== groupsRequestId) {
      return;
    }
    loadError.value = toAppError(cause);
  } finally {
    if (current === groupsRequestId) {
      loading.value = false;
    }
  }
}

async function createGroup() {
  if (loading.value || creating.value || loadError.value) {
    return;
  }

  const name = newGroupName.value.trim();
  const issue = groupNameIssue(
    name,
    groups.value,
    null,
    t("favorites.defaultGroup"),
  );
  if (issue) {
    nameError.value = t(GROUP_NAME_ISSUE_KEYS[issue]);
    return;
  }

  creating.value = true;
  nameError.value = "";

  try {
    const created = await createFavoriteGroup(name);
    groups.value = [...groups.value, created];
    selectedGroupId.value = created.id;
    newGroupName.value = "";
    message.success(t("favorites.group.createdHint"), { duration: 6000 });
  } catch (cause) {
    message.error(formatError(cause));
  } finally {
    creating.value = false;
  }
}

async function confirm() {
  const group = selectedGroup.value;
  if (!group || !canConfirm.value) {
    return;
  }

  saving.value = true;

  try {
    const entry = await addFavorite({
      platform: props.platform,
      id: props.id,
      title: props.title,
      thumb: props.thumb,
      groupId: group.id,
    });
    emit("saved", entry);
    emit("update:show", false);
    message.success(
      t("favorites.group.assigned", { group: groupLabel(group) }),
    );
  } catch (cause) {
    message.error(formatError(cause));
  } finally {
    saving.value = false;
  }
}

function close() {
  if (saving.value) {
    return;
  }
  emit("update:show", false);
}

watch(
  () => props.show,
  (show) => {
    if (!show) {
      groupsRequestId++;
      return;
    }

    newGroupName.value = "";
    nameError.value = "";
    selectedGroupId.value = props.currentGroupId ?? DEFAULT_GROUP_ID;
    void loadGroups();
  },
  { immediate: true },
);

watch(newGroupName, () => {
  nameError.value = "";
});
</script>

<template>
  <AppSheet
    :show="show"
    :title="pickerTitle"
    :width="440"
    :busy="saving"
    @update:show="emit('update:show', $event)"
  >
    <div class="group-picker">
      <div v-if="loading" class="group-picker__loading" aria-busy="true">
        <NSpin size="medium" />
      </div>

      <NAlert v-else-if="loadError" type="error" :bordered="false">
        <div class="group-picker__alert">
          <span>{{ loadErrorText }}</span>
          <NButton size="small" @click="loadGroups">
            {{ t("favorites.retry") }}
          </NButton>
        </div>
      </NAlert>

      <template v-else>
        <NRadioGroup
          v-model:value="selectedGroupId"
          class="group-picker__list"
          :aria-label="pickerTitle"
        >
          <NRadio
            v-for="group in groups"
            :key="group.id"
            :value="group.id"
            class="group-picker__option"
          >
            <span class="group-picker__label">
              <span class="group-picker__name">{{ groupLabel(group) }}</span>
              <span class="group-picker__count">{{ group.count }}</span>
            </span>
          </NRadio>
        </NRadioGroup>

        <div class="group-picker__create">
          <NInput
            v-model:value="newGroupName"
            :placeholder="t('favorites.group.namePlaceholder')"
            :maxlength="GROUP_NAME_MAX_LENGTH"
            :status="nameError ? 'error' : undefined"
            @keyup.enter="createGroup"
          />
          <NButton
            :loading="creating"
            :disabled="loading || creating || loadError !== null"
            @click="createGroup"
          >
            {{ t("favorites.group.create") }}
          </NButton>
        </div>
        <p v-if="nameError" class="group-picker__error">{{ nameError }}</p>
      </template>
    </div>

    <template #footer>
      <NButton :disabled="saving" @click="close">
        {{ t("favorites.cancel") }}
      </NButton>
      <NButton
        type="primary"
        :loading="saving"
        :disabled="!canConfirm"
        @click="confirm"
      >
        {{ t("favorites.group.confirm") }}
      </NButton>
    </template>
  </AppSheet>
</template>

<style scoped>
.group-picker {
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
}

.group-picker__loading {
  display: flex;
  justify-content: center;
  padding: 1.5rem 0;
}

.group-picker__alert {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.75rem;
  flex-wrap: wrap;
}

/* The sheet body already scrolls, so the list grows and the panel caps the
   height instead of trapping touch gestures in a nested scroll area. */
.group-picker__list {
  display: flex;
  flex-direction: column;
  gap: 0.1rem;
}

.group-picker__option {
  width: 100%;
  min-height: 44px;
  padding: 0.5rem 0.25rem;
  border-radius: 8px;
}

.group-picker__option:hover {
  background-color: rgba(127, 127, 127, 0.08);
}

.group-picker__option :deep(.n-radio__label) {
  flex: 1;
  min-width: 0;
}

.group-picker__label {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.75rem;
  min-width: 0;
  overflow-wrap: anywhere;
}

.group-picker__name {
  min-width: 0;
}

.group-picker__count {
  flex: none;
  font-size: 0.75rem;
  opacity: 0.55;
  font-variant-numeric: tabular-nums;
}

.group-picker__create {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 0.5rem;
}

.group-picker__create :deep(.n-input) {
  flex: 1 1 10rem;
  min-width: 0;
}

.group-picker__error {
  margin: -0.25rem 0 0;
  font-size: 0.78rem;
  color: #d03050;
}
</style>
