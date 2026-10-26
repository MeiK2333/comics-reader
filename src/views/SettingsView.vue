<script setup lang="ts">
import { computed, reactive, ref } from "vue";
import { useBackNavigation } from "../composables/useBackNavigation";
import { useMessage } from "naive-ui";
import type { SelectOption } from "naive-ui";
import HomeIcon from "../components/HomeIcon.vue";
import { beginEchStartup, setEchEnabled } from "../api/ech";
import { setLoggingEnabled } from "../api/logs";
import {
  applyProxy,
  validateProxy,
  type ProxyValidationError,
} from "../api/proxy";
import { toAppError } from "../errors";
import { useErrorMessage } from "../composables/useErrorMessage";
import { useAppI18n, type MessageKey } from "../i18n";
import { useSettingsStore, type ProxySettings } from "../stores/settings";

const { goBack } = useBackNavigation();
const { t } = useAppI18n();
const settings = useSettingsStore();
const message = useMessage();
const formatError = useErrorMessage();

const themeOptions = computed<SelectOption[]>(() => [
  { label: t("settings.theme.options.system"), value: "system" },
  { label: t("settings.theme.options.light"), value: "light" },
  { label: t("settings.theme.options.dark"), value: "dark" },
]);

const localeOptions = computed<SelectOption[]>(() => [
  { label: t("settings.language.options.system"), value: "system" },
  { label: t("settings.language.options.en"), value: "en" },
  { label: t("settings.language.options.zh-CN"), value: "zh-CN" },
]);

const readerDirectionOptions = computed<SelectOption[]>(() => [
  {
    label: t("settings.readerDirection.options.ltr"),
    value: "ltr",
  },
  {
    label: t("settings.readerDirection.options.rtl"),
    value: "rtl",
  },
  {
    label: t("settings.readerDirection.options.ttb"),
    value: "ttb",
  },
]);

// The form edits a copy so a rejected address is never persisted. Only a
// successful `set_http_proxy` call writes back to the store.
const proxyDraft = reactive<{
  enabled: boolean;
  host: string;
  port: number | null;
}>({ ...settings.proxy });
const savingProxy = ref(false);
const savingEch = ref(false);
const savingLogging = ref(false);

const PROXY_ERROR_KEYS: Record<ProxyValidationError, MessageKey> = {
  hostRequired: "settings.proxy.errors.hostRequired",
  hostInvalid: "settings.proxy.errors.hostInvalid",
  portInvalid: "settings.proxy.errors.portInvalid",
};

function proxyErrorMessage(cause: unknown): string {
  const error = toAppError(cause);
  return error.code === "invalidInput" && error.details.field === "proxy"
    ? t("settings.proxy.errors.invalid")
    : formatError(cause);
}

async function disableProxyNow(): Promise<boolean> {
  const next: ProxySettings = { ...settings.proxy, enabled: false };
  savingProxy.value = true;
  try {
    await applyProxy(next);
    settings.proxy = next;
    proxyDraft.enabled = false;
    return true;
  } catch (cause) {
    message.error(proxyErrorMessage(cause));
    return false;
  } finally {
    savingProxy.value = false;
  }
}

/**
 * Turning the proxy off is always safe to apply immediately: there is no
 * address to validate, and the user may need to fall back to a direct
 * connection right away. Enabling still requires the save button so host and
 * port can be validated first; turning it on also disables ECH because the
 * two network modes are mutually exclusive.
 */
async function onProxyToggle(enabled: boolean) {
  if (savingProxy.value || savingEch.value) {
    return;
  }

  if (!enabled) {
    if (!(await disableProxyNow())) {
      proxyDraft.enabled = true;
    }
    return;
  }

  if (!settings.ech) {
    return;
  }

  savingEch.value = true;
  try {
    const pending = setEchEnabled(false);
    beginEchStartup(pending);
    await pending;
    settings.ech = false;
  } catch (cause) {
    proxyDraft.enabled = false;
    message.error(t("settings.ech.errors.update"));
    console.error("failed to disable ECH for the proxy", cause);
  } finally {
    savingEch.value = false;
  }
}

/** ECH and the custom proxy are mutually exclusive; both sides must confirm. */
async function onEchToggle(enabled: boolean) {
  if (savingEch.value) {
    return;
  }

  savingEch.value = true;
  try {
    if (enabled) {
      if (proxyDraft.enabled || settings.proxy.enabled) {
        const cleared = await disableProxyNow();
        if (!cleared) {
          settings.ech = false;
          return;
        }
      }

      const pending = setEchEnabled(true);
      beginEchStartup(pending);
      await pending;
      settings.ech = true;
    } else {
      const pending = setEchEnabled(false);
      beginEchStartup(pending);
      await pending;
      settings.ech = false;
    }
  } catch (cause) {
    // Revert the v-model value because the backend did not accept the change.
    settings.ech = !enabled;
    message.error(t("settings.ech.errors.update"));
    console.error("failed to update ECH", cause);
  } finally {
    savingEch.value = false;
  }
}

async function onLoggingToggle(enabled: boolean) {
  if (savingLogging.value) {
    return;
  }

  savingLogging.value = true;
  try {
    await setLoggingEnabled(enabled);
    settings.logging = enabled;
  } catch (cause) {
    // The file could not be opened (or removed); keep the store in sync with
    // what Rust actually accepted instead of persisting a broken preference.
    settings.logging = !enabled;
    message.error(t("settings.logging.errors.update"));
    console.error("failed to update logging", cause);
  } finally {
    savingLogging.value = false;
  }
}

async function saveProxy() {
  if (savingEch.value) {
    return;
  }

  const validationError = proxyDraft.enabled
    ? validateProxy(proxyDraft.host, proxyDraft.port)
    : null;
  if (validationError) {
    message.error(t(PROXY_ERROR_KEYS[validationError]));
    return;
  }

  const next: ProxySettings = {
    enabled: proxyDraft.enabled,
    host: proxyDraft.host.trim(),
    port: proxyDraft.port ?? settings.proxy.port,
  };

  savingProxy.value = true;
  try {
    await applyProxy(next);
    settings.proxy = next;
    message.success(t("settings.proxy.saved"));
  } catch (cause) {
    message.error(proxyErrorMessage(cause));
  } finally {
    savingProxy.value = false;
  }
}

function goHome() {
  goBack({ name: "home" });
}
</script>

<template>
  <main class="settings-page">
    <header class="settings-header">
      <NButton
        attr-type="button"
        quaternary
        circle
        size="large"
        :aria-label="t('settings.back')"
        @click="goHome"
      >
        <template #icon>
          <HomeIcon />
        </template>
      </NButton>
      <h1>{{ t("settings.title") }}</h1>
    </header>

    <NCard class="settings-card" size="small">
      <div class="setting">
        <span>{{ t("settings.theme.label") }}</span>
        <NSelect
          v-model:value="settings.theme"
          class="setting-select"
          :options="themeOptions"
        />
      </div>
      <div class="setting">
        <span>{{ t("settings.language.label") }}</span>
        <NSelect
          v-model:value="settings.locale"
          class="setting-select"
          :options="localeOptions"
        />
      </div>
      <div class="setting">
        <span>{{ t("settings.readerDirection.label") }}</span>
        <NSelect
          v-model:value="settings.readerDirection"
          class="setting-select"
          :options="readerDirectionOptions"
        />
      </div>

      <NDivider class="settings-divider" />

      <h2 class="settings-subtitle">{{ t("settings.proxy.title") }}</h2>

      <div class="setting">
        <span>{{ t("settings.proxy.enable") }}</span>
        <NSwitch
          v-model:value="proxyDraft.enabled"
          :loading="savingProxy"
          :disabled="savingProxy || savingEch"
          @update:value="onProxyToggle"
        />
      </div>

      <div class="proxy-fields">
        <div class="proxy-field">
          <span class="proxy-field__label">{{ t("settings.proxy.host") }}</span>
          <NInput
            v-model:value="proxyDraft.host"
            :disabled="!proxyDraft.enabled || savingEch"
            :placeholder="t('settings.proxy.hostPlaceholder')"
            spellcheck="false"
          />
        </div>
        <div class="proxy-field proxy-field--port">
          <span class="proxy-field__label">{{ t("settings.proxy.port") }}</span>
          <NInputNumber
            v-model:value="proxyDraft.port"
            :disabled="!proxyDraft.enabled || savingEch"
            :min="1"
            :max="65535"
            :precision="0"
            :show-button="false"
          />
        </div>
      </div>

      <div v-if="proxyDraft.enabled" class="proxy-actions">
        <NButton
          attr-type="button"
          size="small"
          :loading="savingProxy"
          :disabled="savingEch"
          @click="saveProxy"
        >
          {{ t("settings.proxy.save") }}
        </NButton>
      </div>

      <div class="setting ech-setting">
        <span>{{ t("settings.ech.enable") }}</span>
        <NSwitch
          v-model:value="settings.ech"
          :loading="savingEch"
          :disabled="savingEch || savingProxy"
          @update:value="onEchToggle"
        />
      </div>

      <NDivider class="settings-divider" />

      <h2 class="settings-subtitle">{{ t("settings.logging.title") }}</h2>

      <div class="setting">
        <span>{{ t("settings.logging.enable") }}</span>
        <NSwitch
          v-model:value="settings.logging"
          :loading="savingLogging"
          :disabled="savingLogging"
          @update:value="onLoggingToggle"
        />
      </div>

      <RouterLink class="setting setting-link" :to="{ name: 'logs' }">
        <span>{{ t("settings.logging.view") }}</span>
      </RouterLink>
    </NCard>
  </main>
</template>

<style scoped>
.settings-page {
  width: min(560px, calc(100% - 2.5rem));
  margin: 0 auto;
  padding: 1.5rem 1.25rem 4rem;
}

.settings-header {
  display: flex;
  align-items: center;
  gap: 0.35rem;
  margin-bottom: 1rem;
}

.settings-header h1 {
  margin: 0;
  font-size: 1.25rem;
  font-weight: 600;
}

.settings-card {
  margin: 0 auto;
  text-align: left;
}

.setting {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 1rem;
}

.setting + .setting {
  margin-top: 0.75rem;
}

.setting-select {
  width: 9.5rem;
  flex: none;
}

.setting-link {
  color: inherit;
  text-decoration: none;
  padding: 0.4rem 0.5rem;
  margin-right: -0.5rem;
  margin-left: -0.5rem;
  border-radius: 6px;
}

.setting-link:hover {
  background-color: rgba(128, 128, 128, 0.12);
}

.setting-link:focus-visible {
  outline: 2px solid currentColor;
  outline-offset: 1px;
}

.settings-divider {
  margin: 1.25rem 0;
}

.settings-subtitle {
  margin: 0 0 0.75rem;
  font-size: 1rem;
  font-weight: 600;
}

.proxy-fields {
  display: flex;
  align-items: flex-end;
  gap: 0.75rem;
  margin-top: 0.75rem;
}

.proxy-field {
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: 0.35rem;
}

.proxy-field--port {
  flex: 0 0 8rem;
}

.proxy-field__label {
  font-size: 0.85rem;
  opacity: 0.8;
}

.proxy-actions {
  display: flex;
  justify-content: flex-end;
  margin-top: 0.9rem;
}

.ech-setting {
  margin-top: 0.9rem;
}

</style>
