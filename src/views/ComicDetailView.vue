<script setup lang="ts">
import { computed, watch } from "vue";
import { RouterView, useRoute, useRouter } from "vue-router";
import type {
  ComicDetail,
  ComicPlatform,
  JmDetail as JmDetailData,
  ManhuaguiDetail as ManhuaguiDetailData,
  NhentaiDetail as NhentaiDetailData,
} from "../api/comics";
import { isComicPlatform } from "../api/comics";
import FavoriteGroupPicker from "../components/favorites/FavoriteGroupPicker.vue";
import ComicDetailSkeleton from "../components/comic-detail/ComicDetailSkeleton.vue";
import DetailActions from "../components/comic-detail/DetailActions.vue";
import DetailTopbar from "../components/comic-detail/DetailTopbar.vue";
import JmDetail from "../components/comic-detail/JmDetail.vue";
import ManhuaguiDetail from "../components/comic-detail/ManhuaguiDetail.vue";
import NhentaiDetail from "../components/comic-detail/NhentaiDetail.vue";
import { useBackNavigation } from "../composables/useBackNavigation";
import { useComicDetail } from "../composables/useComicDetail";
import { useDetailReturnTarget } from "../composables/useDetailReturnTarget";
import { useDownloadAction } from "../composables/useDownloadAction";
import { useErrorMessage } from "../composables/useErrorMessage";
import {
  useFavoriteAction,
  type FavoriteSnapshot,
} from "../composables/useFavoriteAction";
import { useAppI18n } from "../i18n";
import { setWindowTitle, updateWindowTitle } from "../router";

const route = useRoute();
const router = useRouter();
const { t } = useAppI18n();
const formatError = useErrorMessage();
const { goBack: back } = useBackNavigation();
const { backLabel, backTarget } = useDetailReturnTarget();

const platform = computed<ComicPlatform | null>(() => {
  const value = route.params.platform;
  return isComicPlatform(value) ? value : null;
});

const id = computed(() => {
  const value = route.params.id;
  return typeof value === "string" ? value : "";
});

const comic = useComicDetail({ platform, id });
const {
  detail,
  download,
  loading,
  state,
  manhuaguiChapters,
  downloadChapters,
  canRead,
  canResume,
  resumeChapterId,
  resumePage,
  resumeTitle,
  downloadLoading,
  hasDownload,
  load,
} = comic;

const favoriteSnapshot = computed<FavoriteSnapshot | null>(() => {
  const current = detail.value;
  return current
    ? {
        platform: current.platform,
        title: current.title,
        thumb: current.thumb,
      }
    : null;
});

const favorite = useFavoriteAction({
  platform,
  id,
  detail: favoriteSnapshot,
});
const {
  favorited,
  currentGroupId,
  loading: favoriteLoading,
  pending: favoritePending,
  showGroupPicker,
  favoriteButtonText,
  favoriteActionLabel,
  handlers: favoriteHandlers,
  handleClick: handleFavoriteClick,
  onSaved: onGroupSaved,
} = favorite;

const downloadAction = useDownloadAction({
  platform,
  id,
  detail,
  download,
  downloadLoading,
  downloadChapters,
  latest: comic.latest,
  reloadDownload: comic.reloadDownload,
});
const {
  pending: downloadPending,
  buttonLabel: downloadButtonLabel,
  onDownloadClick,
} = downloadAction;

const onlineDetail = computed<ComicDetail | null>(() =>
  state.value.kind === "online" ? state.value.detail : null,
);
const stateError = computed(() =>
  state.value.kind === "error" ? state.value.error : null,
);
const readerActive = computed(() => route.name === "comic-reader");

const notFound = computed(
  () =>
    stateError.value?.code === "notFound" ||
    stateError.value?.code === "invalidInput",
);
const errorTitle = computed(() =>
  notFound.value ? t("detail.notFound") : t("detail.loadFailed"),
);
const errorText = computed(() => {
  const current = stateError.value;
  if (!current) {
    return "";
  }
  return notFound.value
    ? t("detail.notFoundDescription")
    : formatError(current);
});

const manhuaguiDetail = computed<ManhuaguiDetailData | null>(() =>
  detail.value?.platform === "manhuagui" ? detail.value : null,
);
const jmDetail = computed<JmDetailData | null>(() =>
  detail.value?.platform === "jm" ? detail.value : null,
);
const nhentaiDetail = computed<NhentaiDetailData | null>(() =>
  detail.value?.platform === "nhentai" ? detail.value : null,
);

/**
 * The detail component stays mounted while the nested reader is open, so the
 * reader's dynamic title would otherwise remain after it closes. Restore the
 * detail title whenever this route becomes visible again; no data is reloaded.
 */
function syncTitle() {
  if (route.name !== "comic-detail") {
    return;
  }
  const current = detail.value;
  if (current) {
    setWindowTitle(`${current.title} · ${t("app.name")}`);
    return;
  }
  updateWindowTitle();
}

watch([detail, stateError, () => route.name], syncTitle, { immediate: true });

function goBack() {
  back(backTarget.value);
}

function openDownloads() {
  void router.push({ name: "downloads" });
}

function openChapter(chapterId: string) {
  const currentPlatform = platform.value;
  if (!currentPlatform) {
    return;
  }
  void router.push({
    name: "comic-reader",
    params: {
      platform: currentPlatform,
      id: id.value,
      chapterId,
    },
  });
}

function openReader() {
  const currentPlatform = platform.value;
  const current = detail.value;
  if (!currentPlatform || !current) {
    return;
  }

  // Chaptered platforms start at the first chapter and let the reader navigate
  // from there.
  if (current.platform === "jm") {
    const first = current.extra.chapters[0];
    if (first) {
      openChapter(first.id);
    }
    return;
  }
  if (current.platform === "manhuagui") {
    const first = manhuaguiChapters.value[0];
    if (first) {
      openChapter(first.id);
    }
    return;
  }

  void router.push({
    name: "comic-reader",
    params: { platform: current.platform, id: id.value },
  });
}

function openResume() {
  const currentPlatform = platform.value;
  const entry = comic.historyEntry.value;
  if (!currentPlatform || !entry) {
    return;
  }

  const params: Record<string, string> = {
    platform: currentPlatform,
    id: id.value,
  };
  const chapterId = resumeChapterId.value;
  if (chapterId) {
    params.chapterId = chapterId;
  }
  const page = resumePage.value;

  void router.push({
    name: "comic-reader",
    params,
    query: page > 1 ? { page: String(page) } : {},
  });
}
</script>

<template>
  <main
    class="detail-page"
    :inert="readerActive || undefined"
    :aria-hidden="readerActive || undefined"
  >
    <DetailTopbar
      :back-label="backLabel"
      :loading="loading"
      @back="goBack"
    >
      <DetailActions
        v-if="onlineDetail"
        :show-favorite="true"
        :favorited="favorited"
        :favorite-loading="favoriteLoading"
        :favorite-pending="favoritePending"
        :favorite-action-label="favoriteActionLabel"
        :favorite-button-text="favoriteButtonText"
        :favorite-handlers="favoriteHandlers"
        :show-download="canRead"
        :can-read="canRead"
        :can-resume="canResume"
        :download-loading="downloadLoading"
        :download-pending="downloadPending"
        :download-button-label="downloadButtonLabel"
        :resume-title="resumeTitle"
        @favorite="handleFavoriteClick"
        @download="onDownloadClick"
        @read="openReader"
        @resume="openResume"
      />
    </DetailTopbar>

    <FavoriteGroupPicker
      v-if="onlineDetail"
      v-model:show="showGroupPicker"
      :platform="onlineDetail.platform"
      :id="id"
      :title="onlineDetail.title"
      :thumb="onlineDetail.thumb"
      :favorited="favorited"
      :current-group-id="currentGroupId"
      @saved="onGroupSaved"
    />

    <ComicDetailSkeleton v-if="state.kind === 'loading'" />

    <NResult
      v-else-if="stateError"
      class="detail-result"
      :status="notFound ? '404' : 'error'"
      :title="errorTitle"
      :description="errorText"
    >
      <template #footer>
        <div class="detail-actions">
          <NButton v-if="!notFound" type="primary" @click="load()">
            {{ t("detail.retry") }}
          </NButton>
          <NButton v-if="hasDownload" quaternary @click="openDownloads">
            {{ t("detail.openDownloads") }}
          </NButton>
          <NButton quaternary @click="goBack">{{ backLabel }}</NButton>
        </div>
      </template>
    </NResult>

    <ManhuaguiDetail
      v-else-if="manhuaguiDetail"
      :detail="manhuaguiDetail"
      :resume-chapter-id="resumeChapterId"
      @open-chapter="openChapter"
    />
    <JmDetail
      v-else-if="jmDetail"
      :detail="jmDetail"
      @open-chapter="openChapter"
    />
    <NhentaiDetail v-else-if="nhentaiDetail" :detail="nhentaiDetail" />

    <NResult
      v-else
      status="404"
      :title="t('detail.notFound')"
      :description="t('detail.notFoundDescription')"
    />
  </main>

  <!-- Nested reader: the detail stays mounted underneath. -->
  <RouterView />
</template>

<style scoped>
.detail-page {
  width: min(960px, calc(100% - 2.5rem));
  margin: 0 auto;
  padding: 1.25rem 1.25rem 4rem;
}

.detail-result {
  padding: 2rem 0;
}

.detail-actions {
  display: flex;
  justify-content: center;
  gap: 0.75rem;
}

@media (max-width: 560px) {
  .detail-page {
    padding-top: 0.75rem;
  }
}
</style>
