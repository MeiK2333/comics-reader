<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import type { RouteLocationRaw } from "vue-router";
import { RouterLink } from "vue-router";
import { useMessage } from "naive-ui";
import {
  DEFAULT_GROUP_ID,
  createFavoriteGroup,
  deleteFavoriteGroup,
  listFavoriteGroups,
  listFavorites,
  removeFavorite,
  renameFavoriteGroup,
  type FavoriteEntry,
  type FavoriteGroup,
} from "../api/favorites";
import AppSheet from "../components/AppSheet.vue";
import ComicThumb from "../components/ComicThumb.vue";
import TrashIcon from "../components/TrashIcon.vue";
import FavoriteGroupBar from "../components/favorites/FavoriteGroupBar.vue";
import {
  GROUP_NAME_ISSUE_KEYS,
  GROUP_NAME_MAX_LENGTH,
  groupNameIssue,
} from "../components/favorites/favorite-group-name";
import { useBackNavigation } from "../composables/useBackNavigation";
import { useErrorMessage } from "../composables/useErrorMessage";
import { useLatestRequest } from "../composables/useLatestRequest";
import { usePagedResource } from "../composables/usePagedResource";
import { useDateTimeFormatter } from "../i18n/format";
import { platformLabel } from "../platforms";
import { useAppI18n } from "../i18n";
import { toAppError, type AppError } from "../errors";

const { goBack } = useBackNavigation();
const message = useMessage();
const { t } = useAppI18n();
const formatError = useErrorMessage();
const latest = useLatestRequest();
const groupsLatest = useLatestRequest();
const { format: formatTimestamp, toIso: isoTimestamp } =
  useDateTimeFormatter();

const selectedGroupId = ref<string | null>(null);
const {
  entries,
  total,
  page,
  pageCount,
  loading,
  error: loadError,
  load: loadPage,
  reload,
  reset: resetPage,
  changePage,
} = usePagedResource<FavoriteEntry>({
  fetch: (offset, limit) =>
    listFavorites(offset, limit, selectedGroupId.value),
});
const removingKey = ref<string | null>(null);

const groups = ref<FavoriteGroup[]>([]);
const groupsTotal = ref(0);
const groupsLoading = ref(true);
const groupsError = ref<AppError | null>(null);

const groupDialogShow = ref(false);
const groupDialogMode = ref<"create" | "rename">("create");
const groupDialogGroupId = ref<string | null>(null);
const groupDialogName = ref("");
const groupDialogError = ref("");
const groupDialogSaving = ref(false);
const showGroupManager = ref(false);
const deletingGroupId = ref<string | null>(null);

const countText = computed(() => t("favorites.count", { count: total.value }));
const errorText = computed(() =>
  loadError.value ? formatError(loadError.value) : "",
);
const groupsErrorText = computed(() =>
  groupsError.value ? formatError(groupsError.value) : "",
);

const customGroups = computed(() =>
  groups.value.filter((group) => group.id !== DEFAULT_GROUP_ID),
);
const defaultGroupCount = computed(
  () =>
    groups.value.find((group) => group.id === DEFAULT_GROUP_ID)?.count ?? 0,
);

const groupDialogTitle = computed(() =>
  groupDialogMode.value === "create"
    ? t("favorites.group.createTitle")
    : t("favorites.group.renameTitle"),
);

function entryKey(entry: FavoriteEntry): string {
  return `${entry.platform}:${entry.id}`;
}

function detailTarget(entry: FavoriteEntry): RouteLocationRaw {
  return {
    name: "comic-detail",
    params: { platform: entry.platform, id: entry.id },
    state: { from: "favorites" },
  };
}

function groupLabelById(groupId: string): string {
  if (groupId === DEFAULT_GROUP_ID) {
    return t("favorites.defaultGroup");
  }
  return (
    groups.value.find((group) => group.id === groupId)?.name ??
    t("favorites.defaultGroup")
  );
}

async function loadGroups() {
  const token = groupsLatest.begin();
  groupsLoading.value = true;
  groupsError.value = null;

  try {
    const result = await listFavoriteGroups();
    if (!groupsLatest.isCurrent(token)) {
      return;
    }
    groups.value = result.groups;
    groupsTotal.value = result.total;
    if (
      selectedGroupId.value !== null &&
      !result.groups.some((group) => group.id === selectedGroupId.value)
    ) {
      selectedGroupId.value = null;
      void resetPage();
    }
  } catch (cause) {
    if (!groupsLatest.isCurrent(token)) {
      return;
    }
    groupsError.value = toAppError(cause);
  } finally {
    if (groupsLatest.isCurrent(token)) {
      groupsLoading.value = false;
    }
  }
}

function selectGroup(groupId: string | null) {
  if (selectedGroupId.value === groupId) {
    return;
  }
  selectedGroupId.value = groupId;
  // Pagination sits below a long list; start the filtered list from the top.
  if (window.scrollY > 0) {
    window.scrollTo({ top: 0, behavior: "smooth" });
  }
  void resetPage();
}

async function remove(entry: FavoriteEntry) {
  if (loading.value || removingKey.value !== null) {
    return;
  }

  const key = entryKey(entry);
  const token = latest.begin();
  removingKey.value = key;

  try {
    await removeFavorite(entry.platform, entry.id);
    if (!latest.isCurrent(token)) {
      return;
    }
    entries.value = entries.value.filter((item) => entryKey(item) !== key);
    total.value = Math.max(0, total.value - 1);
    message.success(t("favorites.removedToast"));

    // A removed row leaves a gap on the current page unless it was the last
    // row overall, so reload to backfill and clamp the page if needed.
    await loadPage();
    await loadGroups();
  } catch (cause) {
    if (latest.isCurrent(token)) {
      message.error(formatError(cause));
    }
  } finally {
    removingKey.value = null;
  }
}

function openCreateGroup() {
  groupDialogMode.value = "create";
  groupDialogGroupId.value = null;
  groupDialogName.value = "";
  groupDialogError.value = "";
  groupDialogShow.value = true;
}

function openCreateGroupFromManager() {
  showGroupManager.value = false;
  openCreateGroup();
}

function openRenameGroup(group: FavoriteGroup) {
  showGroupManager.value = false;
  groupDialogMode.value = "rename";
  groupDialogGroupId.value = group.id;
  groupDialogName.value = group.name;
  groupDialogError.value = "";
  groupDialogShow.value = true;
}

function validateGroupName(name: string): string {
  const issue = groupNameIssue(
    name,
    groups.value,
    groupDialogGroupId.value,
    t("favorites.defaultGroup"),
  );
  return issue ? t(GROUP_NAME_ISSUE_KEYS[issue]) : "";
}

async function saveGroup() {
  if (groupDialogSaving.value) {
    return;
  }

  const name = groupDialogName.value.trim();
  const error = validateGroupName(name);
  if (error) {
    groupDialogError.value = error;
    return;
  }

  groupDialogSaving.value = true;

  try {
    if (groupDialogMode.value === "create") {
      const created = await createFavoriteGroup(name);
      groupDialogShow.value = false;
      selectedGroupId.value = created.id;
      await loadGroups();
      await resetPage();
      message.success(t("favorites.group.createdHint"), { duration: 6000 });
    } else {
      const groupId = groupDialogGroupId.value;
      if (!groupId) {
        return;
      }
      await renameFavoriteGroup(groupId, name);
      groupDialogShow.value = false;
      await loadGroups();
      message.success(t("favorites.group.renamed"));
    }
  } catch (cause) {
    message.error(formatError(cause));
  } finally {
    groupDialogSaving.value = false;
  }
}

async function deleteGroup(group: FavoriteGroup) {
  if (deletingGroupId.value !== null) {
    return;
  }

  deletingGroupId.value = group.id;

  try {
    await deleteFavoriteGroup(group.id);
    const wasSelected = selectedGroupId.value === group.id;
    if (wasSelected) {
      selectedGroupId.value = null;
    }
    await loadGroups();
    if (wasSelected) {
      await resetPage();
    } else {
      await loadPage();
    }
    message.success(t("favorites.group.deleted"));
  } catch (cause) {
    message.error(formatError(cause));
  } finally {
    deletingGroupId.value = null;
  }
}

function goHome() {
  goBack({ name: "home" });
}

watch(groupDialogName, () => {
  groupDialogError.value = "";
});

onMounted(() => {
  void loadGroups();
  void loadPage();
});
</script>

<template>
  <main class="favorites-page">
    <header class="favorites-header">
      <NButton
        attr-type="button"
        quaternary
        circle
        size="large"
        class="favorites-back"
        :aria-label="t('favorites.back')"
        :title="t('favorites.back')"
        @click="goHome"
      >
        <template #icon>
          <svg
            class="favorites-back-icon"
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
      <h1>{{ t("favorites.title") }}</h1>
    </header>

    <FavoriteGroupBar
      :groups="groups"
      :total="groupsTotal"
      :selected-group-id="selectedGroupId"
      :loading="groupsLoading"
      @select="selectGroup"
      @create="openCreateGroup"
      @manage="showGroupManager = true"
    />

    <NCard class="favorites-card" size="small">
      <NAlert
        v-if="groupsError"
        class="favorites-group-alert"
        type="error"
        :bordered="false"
      >
        <div class="favorites-alert__body">
          <span>{{ groupsErrorText }}</span>
          <NButton size="small" @click="loadGroups">
            {{ t("favorites.retry") }}
          </NButton>
        </div>
      </NAlert>

      <div class="favorites-toolbar">
        <span class="favorites-count">{{ countText }}</span>
      </div>

      <NDivider class="favorites-divider" />

      <NAlert
        v-if="loadError"
        class="favorites-alert"
        type="error"
        :bordered="false"
      >
        <div class="favorites-alert__body">
          <span>{{ errorText }}</span>
          <NButton size="small" @click="reload">
            {{ t("favorites.retry") }}
          </NButton>
        </div>
      </NAlert>

      <div
        v-if="loading && !entries.length"
        class="favorites-loading"
        aria-busy="true"
      >
        <NSpin size="medium" />
      </div>

      <NEmpty
        v-else-if="!entries.length && !loadError"
        class="favorites-empty"
        :show-description="selectedGroupId !== null"
        :description="
          selectedGroupId !== null
            ? t('favorites.group.emptyDescription')
            : undefined
        "
      />

      <div v-else-if="entries.length" class="favorites-list">
        <div
          v-for="entry in entries"
          :key="entryKey(entry)"
          class="favorite-entry"
        >
          <RouterLink class="favorite-entry__main" :to="detailTarget(entry)">
            <span class="favorite-entry__cover">
              <ComicThumb
                :platform="entry.platform"
                :thumb="entry.thumb ?? ''"
                :title="entry.title"
              />
            </span>

            <span class="favorite-entry__body">
              <span class="favorite-entry__title" :title="entry.title">
                {{ entry.title }}
              </span>
              <span class="favorite-entry__meta">
                <NTag size="tiny" :bordered="false">
                  {{ platformLabel(entry.platform) }}
                </NTag>
                <NTag size="tiny" :bordered="false">
                  {{ groupLabelById(entry.groupId) }}
                </NTag>
                <time :datetime="isoTimestamp(entry.addedAt)">
                  {{
                    t("favorites.addedAt", {
                      date: formatTimestamp(entry.addedAt),
                    })
                  }}
                </time>
              </span>
            </span>
          </RouterLink>

          <NButton
            attr-type="button"
            quaternary
            circle
            size="small"
            class="favorite-entry__remove"
            :loading="removingKey === entryKey(entry)"
            :disabled="loading || removingKey !== null"
            :aria-label="t('favorites.remove')"
            :title="t('favorites.remove')"
            @click="remove(entry)"
          >
            <template #icon>
              <TrashIcon />
            </template>
          </NButton>
        </div>
      </div>

      <div v-if="pageCount > 1" class="favorites-pagination">
        <NPagination
          :page="page"
          :page-count="pageCount"
          :page-slot="5"
          :disabled="loading || removingKey !== null"
          @update:page="changePage"
        />
      </div>
    </NCard>

    <AppSheet
      v-model:show="groupDialogShow"
      :title="groupDialogTitle"
      :width="420"
      :busy="groupDialogSaving"
    >
      <div class="group-dialog">
        <NInput
          v-model:value="groupDialogName"
          :placeholder="t('favorites.group.namePlaceholder')"
          :maxlength="GROUP_NAME_MAX_LENGTH"
          :status="groupDialogError ? 'error' : undefined"
          @keyup.enter="saveGroup"
        />
        <p v-if="groupDialogError" class="group-dialog__error">
          {{ groupDialogError }}
        </p>
      </div>
      <template #footer>
        <NButton :disabled="groupDialogSaving" @click="groupDialogShow = false">
          {{ t("favorites.cancel") }}
        </NButton>
        <NButton type="primary" :loading="groupDialogSaving" @click="saveGroup">
          {{ t("favorites.group.save") }}
        </NButton>
      </template>
    </AppSheet>

    <AppSheet
      v-model:show="showGroupManager"
      :title="t('favorites.group.manage')"
      :width="520"
    >
      <div class="group-manager">
        <div class="group-manager__row group-manager__row--default">
          <span class="group-manager__name">
            {{ t("favorites.defaultGroup") }}
          </span>
          <span class="group-manager__count">{{ defaultGroupCount }}</span>
          <NTag size="tiny" :bordered="false">
            {{ t("favorites.group.builtIn") }}
          </NTag>
        </div>

        <NEmpty
          v-if="!customGroups.length"
          class="group-manager__empty"
          :description="t('favorites.group.noCustomGroups')"
        />

        <div
          v-for="group in customGroups"
          :key="group.id"
          class="group-manager__row"
        >
          <span class="group-manager__name">{{ group.name }}</span>
          <span class="group-manager__count">{{ group.count }}</span>
          <div class="group-manager__actions">
            <NButton
              size="small"
              quaternary
              :disabled="deletingGroupId !== null"
              @click="openRenameGroup(group)"
            >
              {{ t("favorites.group.rename") }}
            </NButton>
            <NPopconfirm
              placement="left"
              :positive-text="t('favorites.group.delete')"
              :negative-text="t('favorites.cancel')"
              @positive-click="deleteGroup(group)"
            >
              <template #trigger>
                <NButton
                  size="small"
                  type="error"
                  quaternary
                  :loading="deletingGroupId === group.id"
                  :disabled="deletingGroupId !== null"
                >
                  {{ t("favorites.group.delete") }}
                </NButton>
              </template>
              {{ t("favorites.group.deleteConfirm") }}
            </NPopconfirm>
          </div>
        </div>
      </div>

      <template #footer>
        <NButton type="primary" @click="openCreateGroupFromManager">
          + {{ t("favorites.group.create") }}
        </NButton>
      </template>
    </AppSheet>
  </main>
</template>

<style scoped>
.favorites-page {
  width: min(900px, calc(100% - 2.5rem));
  margin: 0 auto;
  padding: 1.25rem 1.25rem 4rem;
}

.favorites-header {
  display: flex;
  align-items: center;
  gap: 0.35rem;
  margin-bottom: 0.75rem;
}

.favorites-back {
  flex: none;
  margin-left: -0.35rem;
}

.favorites-back-icon {
  display: block;
  width: 20px;
  height: 20px;
}

.favorites-header h1 {
  margin: 0;
  font-size: 1.25rem;
  font-weight: 600;
}

.favorites-card {
  margin: 0 auto;
  text-align: left;
}

.favorites-group-alert {
  margin-bottom: 0.5rem;
}

.favorites-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 1rem;
  flex-wrap: wrap;
}

.favorites-count {
  font-size: 0.8rem;
  opacity: 0.7;
}

.favorites-divider {
  margin: 1rem 0;
}

.favorites-alert {
  margin-bottom: 0.75rem;
}

.favorites-alert__body {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.75rem;
  flex-wrap: wrap;
}

.favorites-loading {
  display: flex;
  justify-content: center;
  padding: 3rem 0;
}

.favorites-empty {
  padding: 2.5rem 0;
}

.favorites-list {
  display: flex;
  flex-direction: column;
}

.favorite-entry {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  align-items: center;
  gap: 0.35rem;
  border-bottom: 1px solid rgba(128, 128, 128, 0.18);
}

.favorite-entry:last-child {
  border-bottom: none;
}

.favorite-entry__main {
  display: grid;
  grid-template-columns: 64px minmax(0, 1fr);
  align-items: center;
  gap: 0.85rem;
  min-width: 0;
  padding: 0.65rem 0.25rem;
  border-radius: 8px;
  color: inherit;
  text-decoration: none;
  transition: background-color 150ms ease;
}

.favorite-entry__main:hover {
  background-color: rgba(127, 127, 127, 0.08);
}

.favorite-entry__main:focus-visible {
  outline: 2px solid currentColor;
  outline-offset: 2px;
}

.favorite-entry__cover {
  width: 64px;
  aspect-ratio: 3 / 4;
  overflow: hidden;
  border-radius: 6px;
  background-color: rgba(127, 127, 127, 0.08);
}

.favorite-entry__cover :deep(.comic-thumb) {
  height: 100%;
}

.favorite-entry__cover :deep(.comic-thumb img) {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.favorite-entry__cover :deep(.comic-thumb__placeholder) {
  height: 100%;
  aspect-ratio: auto;
  font-size: 1.4rem;
}

.favorite-entry__body {
  display: flex;
  flex-direction: column;
  gap: 0.2rem;
  min-width: 0;
}

.favorite-entry__title {
  display: -webkit-box;
  overflow: hidden;
  font-size: 0.95rem;
  font-weight: 600;
  line-height: 1.35;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 2;
}

.favorite-entry__meta {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 0.4rem;
  font-size: 0.76rem;
  opacity: 0.72;
}

.favorite-entry__meta time {
  font-variant-numeric: tabular-nums;
}

.favorite-entry__remove {
  flex: none;
  margin-right: 0.25rem;
}

.favorites-pagination {
  display: flex;
  justify-content: center;
  margin-top: 1rem;
}

.group-dialog {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
}

.group-dialog__error {
  margin: 0;
  font-size: 0.78rem;
  color: #d03050;
}

.group-manager {
  display: flex;
  flex-direction: column;
  gap: 0.35rem;
}

.group-manager__row {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  min-width: 0;
  padding: 0.5rem 0.35rem;
  border-bottom: 1px solid rgba(128, 128, 128, 0.16);
}

.group-manager__row:last-of-type {
  border-bottom: none;
}

.group-manager__row--default {
  opacity: 0.75;
}

.group-manager__empty {
  padding: 1.5rem 0;
}

.group-manager__name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.group-manager__count {
  flex: none;
  min-width: 1.5rem;
  text-align: right;
  font-size: 0.78rem;
  opacity: 0.6;
  font-variant-numeric: tabular-nums;
}

.group-manager__actions {
  display: flex;
  flex: none;
  align-items: center;
  gap: 0.25rem;
}

@media (max-width: 560px) {
  .favorites-page {
    /* `width: 100%` would overflow by the page padding: there is no global
       box-sizing reset, so let the block size itself instead. */
    width: auto;
    padding: 0.5rem 0.75rem 3rem;
  }

  .favorites-header {
    margin-bottom: 0.25rem;
  }

  /* Reclaim the double inset: page padding plus Naive's card padding left
     only ~288px of content on a 360px screen. */
  .favorites-card :deep(.n-card__content) {
    padding: 10px;
  }

  .favorite-entry__main {
    grid-template-columns: 52px minmax(0, 1fr);
    gap: 0.65rem;
  }

  .favorite-entry__cover {
    width: 52px;
  }

  .favorite-entry__meta time {
    width: 100%;
  }

  .favorite-entry__remove {
    width: 40px;
    height: 40px;
  }

  .favorites-loading {
    padding: 2rem 0;
  }

  .favorites-empty {
    padding: 2rem 0;
  }

  /* Four items per row do not fit on a phone; move the actions to their own
     right-aligned line. */
  .group-manager__row {
    flex-wrap: wrap;
  }

  .group-manager__actions {
    width: 100%;
    justify-content: flex-end;
  }
}
</style>
