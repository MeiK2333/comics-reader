<script setup lang="ts">
import { computed, nextTick, onMounted, ref } from "vue";
import { useBackNavigation } from "../composables/useBackNavigation";
import { useMessage } from "naive-ui";
import type { ScrollbarInst } from "naive-ui";
import {
  clearLogs,
  exportLogs,
  getLogs,
  type LogLevel,
  type LogSnapshot,
} from "../api/logs";
import LogActionIcon from "../components/LogActionIcon.vue";
import { useErrorMessage } from "../composables/useErrorMessage";
import { useAppI18n, type MessageKey } from "../i18n";

const { goBack: back } = useBackNavigation();
const { t } = useAppI18n();
const message = useMessage();
const formatError = useErrorMessage();

const snapshot = ref<LogSnapshot | null>(null);
const loading = ref(false);
const clearing = ref(false);
const exporting = ref(false);
const loadError = ref<string | null>(null);
const logScrollbar = ref<ScrollbarInst | null>(null);

const entries = computed(() => snapshot.value?.entries ?? []);
const displayEntries = computed(() => [...entries.value].reverse());
const statusText = computed(() => {
  if (!snapshot.value) {
    return "—";
  }
  return snapshot.value.enabled ? t("logs.enabled") : t("logs.disabled");
});
const statusType = computed<"success" | "default">(() =>
  snapshot.value?.enabled ? "success" : "default",
);
const countText = computed(() =>
  t("logs.count", { count: entries.value.length }),
);

const LEVEL_LABEL_KEYS: Record<LogLevel, MessageKey> = {
  info: "logs.levels.info",
  warn: "logs.levels.warn",
  error: "logs.levels.error",
};
const LEVEL_TAG_TYPES: Record<LogLevel, "info" | "warning" | "error"> = {
  info: "info",
  warn: "warning",
  error: "error",
};

const timestampFormatter = new Intl.DateTimeFormat(undefined, {
  year: "numeric",
  month: "2-digit",
  day: "2-digit",
  hour: "2-digit",
  minute: "2-digit",
  second: "2-digit",
  hour12: false,
});

function formatTimestamp(timestamp: number): string {
  return timestampFormatter.format(new Date(timestamp));
}

function levelLabel(level: LogLevel): string {
  return t(LEVEL_LABEL_KEYS[level]);
}

function levelTagType(level: LogLevel): "info" | "warning" | "error" {
  return LEVEL_TAG_TYPES[level];
}

async function load() {
  if (loading.value) {
    return;
  }

  loading.value = true;
  try {
    snapshot.value = await getLogs();
    loadError.value = null;
    // The backend returns oldest-to-newest, but the viewer puts the newest
    // entry at the top. A refresh should therefore return to the top.
    await nextTick();
    logScrollbar.value?.scrollTo({ top: 0 });
  } catch (cause) {
    loadError.value = `${t("logs.loadFailed")} ${formatError(cause)}`;
  } finally {
    loading.value = false;
  }
}

async function clear() {
  if (clearing.value) {
    return;
  }

  clearing.value = true;
  try {
    await clearLogs();
    message.success(t("logs.cleared"));
    await load();
  } catch (cause) {
    message.error(`${t("logs.clearFailed")} ${formatError(cause)}`);
  } finally {
    clearing.value = false;
  }
}

async function exportToFile() {
  if (exporting.value) {
    return;
  }

  exporting.value = true;
  try {
    if (await exportLogs()) {
      message.success(t("logs.exported"));
    }
  } catch (cause) {
    message.error(`${t("logs.exportFailed")} ${formatError(cause)}`);
  } finally {
    exporting.value = false;
  }
}

function goBack() {
  back({ name: "settings" });
}

onMounted(() => {
  void load();
});
</script>

<template>
  <main class="logs-page">
    <header class="logs-header">
      <NButton
        attr-type="button"
        quaternary
        circle
        size="large"
        class="logs-back"
        :aria-label="t('logs.back')"
        :title="t('logs.back')"
        @click="goBack"
      >
        <template #icon>
          <LogActionIcon name="back" />
        </template>
      </NButton>
      <h1>{{ t("logs.title") }}</h1>
    </header>

    <NCard class="logs-card" size="small">
      <div class="logs-toolbar">
        <div class="logs-status">
          <NTag size="small" :bordered="false" :type="statusType">
            {{ statusText }}
          </NTag>
          <span class="logs-count">{{ countText }}</span>
        </div>

        <div class="logs-actions">
          <NButton
            attr-type="button"
            quaternary
            circle
            size="small"
            :loading="loading"
            :aria-label="t('logs.refresh')"
            :title="t('logs.refresh')"
            @click="load"
          >
            <template #icon>
              <LogActionIcon name="refresh" />
            </template>
          </NButton>
          <NButton
            attr-type="button"
            quaternary
            circle
            size="small"
            :loading="exporting"
            :disabled="!snapshot"
            :aria-label="t('logs.export')"
            :title="t('logs.export')"
            @click="exportToFile"
          >
            <template #icon>
              <LogActionIcon name="export" />
            </template>
          </NButton>
          <NButton
            attr-type="button"
            quaternary
            circle
            size="small"
            type="error"
            :loading="clearing"
            :disabled="!snapshot"
            :aria-label="t('logs.clear')"
            :title="t('logs.clear')"
            @click="clear"
          >
            <template #icon>
              <LogActionIcon name="clear" />
            </template>
          </NButton>
        </div>
      </div>

      <p v-if="snapshot?.truncated" class="logs-notice">
        {{ t("logs.truncated", { count: entries.length }) }}
      </p>

      <NDivider class="logs-divider" />

      <NAlert
        v-if="loadError"
        class="logs-alert"
        type="error"
        :bordered="false"
      >
        {{ loadError }}
      </NAlert>

      <div v-if="loading && !snapshot" class="logs-loading">
        <NSpin size="medium" />
      </div>
      <NEmpty
        v-else-if="snapshot && !entries.length"
        class="logs-empty"
        :description="t('logs.empty')"
      />
      <NScrollbar v-else ref="logScrollbar" class="logs-list">
        <div class="logs-entries" role="log">
          <div
            v-for="(entry, index) in displayEntries"
            :key="`${entry.timestamp}-${index}`"
            class="log-entry"
          >
            <time :datetime="new Date(entry.timestamp).toISOString()">
              {{ formatTimestamp(entry.timestamp) }}
            </time>
            <NTag
              size="small"
              :bordered="false"
              :type="levelTagType(entry.level)"
            >
              {{ levelLabel(entry.level) }}
            </NTag>
            <span class="log-target">{{ entry.target }}</span>
            <span class="log-message">{{ entry.message }}</span>
          </div>
        </div>
      </NScrollbar>
    </NCard>
  </main>
</template>

<style scoped>
.logs-page {
  width: min(900px, calc(100% - 2.5rem));
  margin: 0 auto;
  padding: 1.25rem 1.25rem 4rem;
}

.logs-header {
  display: flex;
  align-items: center;
  gap: 0.35rem;
  margin-bottom: 1rem;
}

.logs-back {
  flex: none;
  margin-left: -0.35rem;
}

.logs-header h1 {
  margin: 0;
  font-size: 1.25rem;
  font-weight: 600;
}

.logs-card {
  margin: 0 auto;
  text-align: left;
}

.logs-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 1rem;
  flex-wrap: wrap;
}

.logs-status {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  flex-wrap: wrap;
}

.logs-count {
  font-size: 0.8rem;
  opacity: 0.7;
}

.logs-actions {
  display: flex;
  align-items: center;
  gap: 0.25rem;
}

.logs-notice {
  margin: 0.75rem 0 0;
  font-size: 0.8rem;
  opacity: 0.7;
}

.logs-divider {
  margin: 1rem 0;
}

.logs-alert {
  margin-bottom: 0.75rem;
}

.logs-loading {
  display: flex;
  justify-content: center;
  padding: 3rem 0;
}

.logs-empty {
  padding: 2.5rem 0;
}

.logs-list {
  max-height: min(58vh, 34rem);
}

.logs-entries {
  display: flex;
  flex-direction: column;
}

.log-entry {
  display: grid;
  grid-template-columns: 10.5rem auto 4.5rem minmax(0, 1fr);
  gap: 0.5rem;
  align-items: baseline;
  padding: 0.45rem 0.4rem;
  border-bottom: 1px solid rgba(128, 128, 128, 0.18);
  font-family:
    ui-monospace, SFMono-Regular, "SF Mono", Consolas, "Liberation Mono",
    monospace;
  font-size: 0.76rem;
  line-height: 1.45;
}

.log-entry time {
  font-variant-numeric: tabular-nums;
  opacity: 0.7;
}

.log-target {
  font-weight: 600;
  opacity: 0.8;
  overflow-wrap: anywhere;
}

.log-message {
  white-space: pre-wrap;
  overflow-wrap: anywhere;
}

@media (max-width: 640px) {
  .logs-page {
    padding-top: 0.75rem;
  }

  .log-entry {
    grid-template-columns: 1fr auto;
  }

  .log-entry time {
    grid-column: 1;
  }

  .log-entry > :deep(.n-tag) {
    grid-column: 2;
    justify-self: end;
  }

  .log-target {
    grid-column: 1;
  }

  .log-message {
    grid-column: 1 / -1;
  }
}
</style>
