<template>
  <div class="publish">
    <a-input v-model:value="topic" placeholder="topic（如 sensors/temp）" class="topic" />
    <a-select v-model:value="qos" class="qos">
      <a-select-option value="at_most_once">Q0</a-select-option>
      <a-select-option value="at_least_once">Q1</a-select-option>
      <a-select-option value="exactly_once">Q2</a-select-option>
    </a-select>
    <a-switch v-model:checked="retain" class="retain" checked-children="Retain" un-checked-children="Retain" />
    <a-textarea
      v-model:value="payload"
      placeholder="payload（文本自动按 UTF-8 编码为 base64）"
      :auto-size="{ minRows: 1, maxRows: 3 }"
      class="payload"
    />
    <a-button type="primary" :disabled="!connectionId || !topic" @click="doPublish">发布</a-button>
  </div>
</template>

<script setup lang="ts">
import { message } from 'ant-design-vue'
import { ref } from 'vue'
import { api, encodeBase64 } from '../api/backend'
import type { Qos } from '../types'

const props = defineProps<{ connectionId: string }>()
const topic = ref('')
const payload = ref('')
const qos = ref<Qos>('at_least_once')
const retain = ref(false)

async function doPublish() {
  if (!props.connectionId || !topic.value) return
  try {
    await api.publish(
      props.connectionId,
      topic.value,
      encodeBase64(payload.value),
      qos.value,
      retain.value,
    )
    message.success('已发布')
  } catch (e) {
    message.error('发布失败：' + String(e))
  }
}
</script>

<style scoped>
.publish {
  display: flex;
  align-items: center;
  gap: 8px;
}
.topic {
  width: 260px;
}
.qos {
  width: 72px;
}
.retain {
  flex: none;
}
.payload {
  flex: 1;
}
</style>
