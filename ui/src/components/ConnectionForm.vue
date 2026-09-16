<template>
  <a-modal
    :open="open"
    :title="isEdit ? '编辑连接' : '新建连接'"
    width="720px"
    @ok="submit"
    @cancel="emit('update:open', false)"
    ok-text="保存"
    cancel-text="取消"
    :confirm-loading="saving"
  >
    <a-tabs v-model:activeKey="tab">
      <a-tab-pane key="base" tab="基础">
        <a-form :label-col="{ span: 6 }">
          <a-form-item label="名称"><a-input v-model:value="form.name" placeholder="本地 broker" /></a-form-item>
          <a-form-item label="传输">
            <a-select v-model:value="form.protocol" @change="onProtocolChange">
              <a-select-option value="mqtt">mqtt (TCP)</a-select-option>
              <a-select-option value="mqtts">mqtts (TLS)</a-select-option>
              <a-select-option value="ws">ws</a-select-option>
              <a-select-option value="wss">wss</a-select-option>
            </a-select>
          </a-form-item>
          <a-form-item label="主机"><a-input v-model:value="form.host" /></a-form-item>
          <a-form-item label="端口"><a-input-number v-model:value="form.port" :min="1" :max="65535" /></a-form-item>
          <a-form-item v-if="isWs" label="WS 路径"><a-input v-model:value="form.path" placeholder="/mqtt" /></a-form-item>
          <a-form-item label="协议版本">
            <a-select v-model:value="form.mqtt_version">
              <a-select-option value="v311">MQTT 3.1.1</a-select-option>
              <a-select-option value="v50">MQTT 5.0</a-select-option>
            </a-select>
          </a-form-item>
          <a-form-item label="ClientID"><a-input v-model:value="form.client_id" /></a-form-item>
          <a-form-item label="用户名"><a-input v-model:value="form.username" /></a-form-item>
          <a-form-item label="密码"><a-input-password v-model:value="password" :placeholder="isEdit ? '留空则不修改' : ''" /></a-form-item>
          <a-form-item label="Keep Alive"><a-input-number v-model:value="form.keep_alive" :min="0" /> <span class="hint">秒</span></a-form-item>
          <a-form-item label="Clean Session">
            <a-switch v-model:checked="form.clean_session" />
            <span class="hint" v-if="!form.clean_session"> broker 保留会话，重连不重放订阅</span>
          </a-form-item>
          <a-form-item label="自动连接"><a-switch v-model:checked="form.auto_connect" /></a-form-item>
        </a-form>
      </a-tab-pane>

      <a-tab-pane key="subs" tab="订阅">
        <div class="subs-head">
          <a-input v-model:value="subFilter" placeholder="过滤器，如 sensors/#" style="flex:1" />
          <a-select v-model:value="subQos" style="width:80px">
            <a-select-option value="at_most_once">Q0</a-select-option>
            <a-select-option value="at_least_once">Q1</a-select-option>
            <a-select-option value="exactly_once">Q2</a-select-option>
          </a-select>
          <a-button type="primary" @click="addSub">添加</a-button>
        </div>
        <a-list :data-source="form.subscriptions" size="small" bordered>
          <template #renderItem="{ item, index }">
            <a-list-item>
              <a-tag :color="item.color || 'blue'">{{ item.filter }}</a-tag>
              <span class="qos">{{ qosLabel(item.qos) }}</span>
              <template #actions>
                <a-switch :checked="item.enabled" size="small" @change="(v:boolean)=>item.enabled=v" />
                <a-button size="small" type="link" danger @click="form.subscriptions.splice(index,1)">删</a-button>
              </template>
            </a-list-item>
          </template>
        </a-list>
        <div class="hint">订阅在连接成功后生效；编辑连接保存后，运行中的订阅表随之更新。</div>
      </a-tab-pane>

      <a-tab-pane key="tls" tab="TLS/代理">
        <a-form :label-col="{ span: 6 }">
          <a-form-item label="TLS">
            <a-switch v-model:checked="tlsEnabled" @change="onTlsToggle" />
          </a-form-item>
          <template v-if="tlsEnabled">
            <a-form-item label="CA 证书"><a-input v-model:value="tlsCa" placeholder="/path/ca.pem（留空用系统根证书）" /></a-form-item>
            <a-form-item label="校验主机名"><a-switch v-model:checked="tlsVerify" /></a-form-item>
            <a-form-item label="双向认证">
              <a-switch v-model:checked="mtls" />
            </a-form-item>
            <template v-if="mtls">
              <a-form-item label="客户端证书"><a-input v-model:value="mtlsCert" placeholder="/path/client.pem" /></a-form-item>
              <a-form-item label="客户端私钥"><a-input v-model:value="mtlsKey" placeholder="/path/client.key" /></a-form-item>
            </template>
            <div class="hint" v-if="!tlsVerify">⚠️ 关闭主机名校验将放行自签证书，仅用于内网/测试。</div>
          </template>
          <a-form-item label="HTTP 代理">
            <a-input v-model:value="proxyUrl" placeholder="http://127.0.0.1:7890（留空不走代理）" />
          </a-form-item>
        </a-form>
      </a-tab-pane>

      <a-tab-pane key="will" tab="遗嘱/高级">
        <a-form :label-col="{ span: 6 }">
          <a-form-item label="启用遗嘱"><a-switch v-model:checked="willEnabled" /></a-form-item>
          <template v-if="willEnabled">
            <a-form-item label="遗嘱 topic"><a-input v-model:value="willTopic" /></a-form-item>
            <a-form-item label="遗嘱 payload"><a-input v-model:value="willPayload" /></a-form-item>
            <a-form-item label="遗嘱 QoS">
              <a-select v-model:value="willQos" style="width:100px">
                <a-select-option value="at_most_once">Q0</a-select-option>
                <a-select-option value="at_least_once">Q1</a-select-option>
                <a-select-option value="exactly_once">Q2</a-select-option>
              </a-select>
            </a-form-item>
            <a-form-item label="遗嘱 Retain"><a-switch v-model:checked="willRetain" /></a-form-item>
          </template>
          <template v-if="form.mqtt_version === 'v50'">
            <a-form-item label="会话过期(秒)"><a-input-number v-model:value="m5SessionExpiry" :min="0" /></a-form-item>
            <a-form-item label="主题别名上限"><a-input-number v-model:value="m5TopicAliasMax" :min="0" /></a-form-item>
          </template>
        </a-form>
      </a-tab-pane>
    </a-tabs>
  </a-modal>
</template>

<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue'
import { encodeBase64, decodeBase64 } from '../api/backend'
import type { Connection, Qos, Subscription } from '../types'

const props = defineProps<{ open: boolean; connection?: Connection | null }>()
const emit = defineEmits<{
  (e: 'update:open', v: boolean): void
  (e: 'save', conn: Connection, password?: string): void
}>()

const isEdit = computed(() => !!props.connection)
const tab = ref('base')
const saving = ref(false)

const form = reactive({
  id: '',
  name: '',
  protocol: 'mqtt',
  host: '127.0.0.1',
  port: 1883,
  path: null as string | null,
  client_id: '',
  username: '',
  mqtt_version: 'v311' as 'v311' | 'v50',
  clean_session: true,
  keep_alive: 60,
  auto_connect: false,
  subscriptions: [] as Subscription[],
})
const password = ref('')

// TLS
const tlsEnabled = ref(false)
const tlsCa = ref('')
const tlsVerify = ref(true)
const mtls = ref(false)
const mtlsCert = ref('')
const mtlsKey = ref('')
const proxyUrl = ref('')

// 遗嘱
const willEnabled = ref(false)
const willTopic = ref('')
const willPayload = ref('')
const willQos = ref<Qos>('at_least_once')
const willRetain = ref(false)

// MQTT5
const m5SessionExpiry = ref<number | undefined>(undefined)
const m5TopicAliasMax = ref<number | undefined>(undefined)

// 订阅输入
const subFilter = ref('')
const subQos = ref<Qos>('at_least_once')

const isWs = computed(() => form.protocol === 'ws' || form.protocol === 'wss')

function onProtocolChange() {
  if (form.protocol === 'mqtts') { tlsEnabled.value = true }
  if (form.protocol === 'mqtt') { tlsEnabled.value = false }
}

function onTlsToggle(v: boolean) {
  tlsEnabled.value = v
}

function qosLabel(q: Qos) {
  return { at_most_once: 'Q0', at_least_once: 'Q1', exactly_once: 'Q2' }[q]
}

function addSub() {
  const f = subFilter.value.trim()
  if (!f) return
  form.subscriptions.push({ filter: f, qos: subQos.value, color: null, enabled: true })
  subFilter.value = ''
}

function fillFrom(c: Connection) {
  form.id = c.id
  form.name = c.name
  form.protocol = c.protocol
  form.host = c.host
  form.port = c.port
  form.path = c.path ?? null
  form.client_id = c.client_id
  form.username = c.username
  form.mqtt_version = c.mqtt_version
  form.clean_session = c.clean_session
  form.keep_alive = c.keep_alive
  form.auto_connect = c.auto_connect
  form.subscriptions = c.subscriptions.map((s) => ({ ...s }))
  password.value = ''

  tlsEnabled.value = c.tls.mode === 'tls'
  tlsCa.value = c.tls.ca ?? ''
  tlsVerify.value = c.tls.verify_hostname
  mtls.value = !!c.tls.client_auth
  mtlsCert.value = c.tls.client_auth?.cert ?? ''
  mtlsKey.value = c.tls.client_auth?.key ?? ''
  proxyUrl.value = c.proxy?.url ?? ''

  willEnabled.value = !!c.last_will
  willTopic.value = c.last_will?.topic ?? ''
  willPayload.value = c.last_will ? decodeBase64(c.last_will.payload_base64) : ''
  willQos.value = c.last_will?.qos ?? 'at_least_once'
  willRetain.value = c.last_will?.retain ?? false

  m5SessionExpiry.value = c.properties?.session_expiry_interval ?? undefined
  m5TopicAliasMax.value = c.properties?.topic_alias_max ?? undefined
}

watch(
  () => [props.open, props.connection] as const,
  ([open, conn]) => {
    if (!open) return
    if (conn) fillFrom(conn)
    else {
      form.id = crypto.randomUUID()
      form.name = ''
      form.client_id = 'mqttkit-' + Math.random().toString(16).slice(2, 10)
      form.subscriptions = []
      password.value = ''
    }
  },
  { immediate: true },
)

function submit() {
  const conn: Connection = {
    id: form.id,
    name: form.name || form.host,
    protocol: form.protocol,
    host: form.host,
    port: form.port,
    path: isWs.value ? form.path || '/mqtt' : null,
    client_id: form.client_id,
    username: form.username,
    credential_ref: null,
    mqtt_version: form.mqtt_version,
    clean_session: form.clean_session,
    keep_alive: form.keep_alive,
    tls: tlsEnabled.value
      ? {
          mode: 'tls',
          ca: tlsCa.value || null,
          client_auth: mtls.value ? { cert: mtlsCert.value, key: mtlsKey.value } : null,
          verify_hostname: tlsVerify.value,
        }
      : { mode: 'none', verify_hostname: true },
    proxy: proxyUrl.value ? { url: proxyUrl.value } : null,
    last_will: willEnabled.value
      ? { topic: willTopic.value, payload_base64: encodeBase64(willPayload.value), qos: willQos.value, retain: willRetain.value }
      : null,
    properties:
      form.mqtt_version === 'v50'
        ? { session_expiry_interval: m5SessionExpiry.value ?? null, maximum_packet_size: null, topic_alias_max: m5TopicAliasMax.value ?? null, user_properties: {} }
        : null,
    auto_connect: form.auto_connect,
    subscriptions: form.subscriptions,
  }
  emit('save', conn, password.value || undefined)
  emit('update:open', false)
}
</script>

<style scoped>
.hint { color: #999; font-size: 12px; margin-left: 8px; }
.subs-head { display: flex; gap: 8px; margin-bottom: 8px; }
.qos { color: #888; }
</style>
