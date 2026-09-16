<template>
  <a-modal
    :open="open"
    title="设置"
    width="560px"
    @ok="save"
    @cancel="emit('update:open', false)"
    ok-text="保存"
    cancel-text="取消"
    :confirm-loading="saving"
  >
    <a-form :label-col="{ span: 7 }">
      <a-form-item label="主题">
        <a-select v-model:value="s.theme">
          <a-select-option value="auto">跟随系统</a-select-option>
          <a-select-option value="light">浅色</a-select-option>
          <a-select-option value="dark">深色</a-select-option>
        </a-select>
      </a-form-item>
      <a-form-item label="消息缓冲区">
        <a-input-number v-model:value="s.message_buffer" :min="1000" :max="1000000" style="width:100%" />
      </a-form-item>
      <a-form-item label="推送间隔(ms)">
        <a-input-number v-model:value="s.flush_interval_ms" :min="10" :max="2000" style="width:100%" />
      </a-form-item>
      <a-form-item label="单批推送条数">
        <a-input-number v-model:value="s.flush_batch" :min="10" :max="5000" style="width:100%" />
      </a-form-item>
      <a-form-item label="日志级别">
        <a-select v-model:value="s.log_level">
          <a-select-option value="error">error</a-select-option>
          <a-select-option value="warn">warn</a-select-option>
          <a-select-option value="info">info</a-select-option>
          <a-select-option value="debug">debug</a-select-option>
          <a-select-option value="trace">trace</a-select-option>
        </a-select>
      </a-form-item>
      <a-form-item label="最小化到托盘">
        <a-switch v-model:checked="s.minimize_to_tray" />
      </a-form-item>
      <a-form-item label="匿名遥测">
        <a-switch v-model:checked="s.telemetry_enabled" />
        <div class="hint">默认关闭。开启后仅上报匿名使用统计，可随时关闭。</div>
      </a-form-item>
    </a-form>
  </a-modal>
</template>

<script setup lang="ts">
import { reactive, ref, watch } from 'vue'
import { message } from 'ant-design-vue'
import { useSettingsStore } from '../stores/settings'
import type { AppSettings } from '../types'

const props = defineProps<{ open: boolean }>()
const emit = defineEmits<{ (e: 'update:open', v: boolean): void }>()

const store = useSettingsStore()
const saving = ref(false)
const s = reactive<AppSettings>({ ...store.settings })

watch(
  () => props.open,
  (v) => {
    if (v) Object.assign(s, store.settings)
  },
)

async function save() {
  saving.value = true
  try {
    Object.assign(store.settings, s)
    await store.save()
    message.success('设置已保存')
    emit('update:open', false)
  } catch (e) {
    message.error('保存失败：' + String(e))
  } finally {
    saving.value = false
  }
}
</script>

<style scoped>
.hint { color: #999; font-size: 12px; }
</style>
