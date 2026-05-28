<template>
  <div class="terminal-shell">
    <main class="terminal-panel">
      <header class="terminal-tabs">
        <div class="tabs-scroll">
          <div
            v-for="(s, index) in sessions"
            :key="s.session_id"
            :class="['terminal-tab', { active: activeSession === s.session_id }]"
            role="button"
            tabindex="0"
            @click="switchSession(s.session_id)"
            @keydown.enter.prevent="switchSession(s.session_id)"
          >
            <span :class="['status-dot', dotClass(index)]"></span>
            <input
              v-if="editingSession === s.session_id"
              :ref="el => setRenameInputRef(s.session_id, el)"
              v-model="editingName"
              class="tab-name-input"
              maxlength="32"
              @click.stop
              @dblclick.stop
              @keydown.enter.prevent.stop="commitRename(s)"
              @keydown.esc.prevent.stop="cancelRename"
              @blur="commitRename(s)"
            />
            <span v-else class="tab-title" @dblclick.stop="beginRename(s)">{{ sessionTitle(s, index) }}</span>
            <button class="tab-rename" title="重命名" @click.stop="beginRename(s)">✎</button>
            <button class="tab-close" title="关闭标签" @click.stop="closeSession(s.session_id)">×</button>
          </div>
          <button class="terminal-tab add-tab" title="新建标签" @click="createNewSession">+</button>
        </div>

        <div class="toolbar">
          <div class="clipboard-menu">
            <button class="tool-btn clipboard-trigger" title="全局剪切板" @click="toggleClipboard">
              <span class="tool-icon">▤</span>
              <span>剪切板</span>
              <span v-if="clipboardHistory.length" class="history-count">{{ clipboardHistory.length }}</span>
            </button>
            <div v-if="clipboardOpen" class="clipboard-popover">
              <div class="clipboard-head">
                <span>全局剪切板</span>
                <button class="clipboard-close" title="关闭" @click="clipboardOpen = false">×</button>
              </div>
              <div class="clipboard-actions">
                <button :disabled="!clipboardHistory.length" @click="clearClipboardHistory">清空</button>
              </div>
              <div v-if="!clipboardHistory.length" class="clipboard-empty">暂无粘贴记录</div>
              <div v-else class="clipboard-list">
                <button
                  v-for="item in clipboardHistory"
                  :key="item.id"
                  class="clipboard-item"
                  :title="clipboardItemTitle(item)"
                  @click="pasteClipboardHistoryItem(item)"
                >
                  <span :class="['clipboard-kind', item.kind]">{{ item.kind === 'image' ? '图片' : '文本' }}</span>
                  <span class="clipboard-preview">{{ clipboardItemPreview(item) }}</span>
                  <span class="clipboard-time">{{ formatClipboardTime(item.createdAt) }}</span>
                </button>
              </div>
            </div>
          </div>
          <button class="tool-btn" title="新建标签" @click="createNewSession">
            <span class="tool-icon">⊞</span>
            <span>新建标签</span>
          </button>
          <button class="tool-btn" title="设置" @click="router.push('/config')">
            <span class="tool-icon">⚙</span>
            <span>设置</span>
          </button>
          <button class="icon-action" title="新网页打开" @click="openInBrowser">↗</button>
        </div>
      </header>

      <section class="terminal-viewport" ref="termContainer">
        <div v-if="sessions.length === 0" class="empty-state">
          <div class="empty-title">FnRmux</div>
          <div class="empty-line">没有正在运行的终端会话</div>
          <button class="primary-command" @click="createNewSession">开启本地终端</button>
        </div>

        <div
          v-for="s in sessions"
          :key="s.session_id"
          :class="['term-wrapper', { active: activeSession === s.session_id }]"
          :ref="el => setTermRef(s.session_id, el)"
        ></div>
      </section>

      <footer class="statusbar">
        <div class="status-left">
          <span class="status-dot online"></span>
          <span>{{ activeSession ? '已连接' : '未连接' }}</span>
          <span v-if="activePath" class="status-path" :title="activePath">{{ activePath }}</span>
        </div>
        <div class="status-center">
          <span class="repo-label">仓库地址:</span>
          <a :href="repoHref" target="_blank" rel="noopener noreferrer" title="https://github.com/user-no-found/FnRmux">
            https://github.com/user-no-found/FnRmux
          </a>
        </div>
        <div class="status-right">
          <span>{{ activeMeta }}</span>
          <span>xterm-256color</span>
          <span class="lock-icon">▣</span>
        </div>
      </footer>
    </main>

    <div v-if="toastMsg" class="paste-toast">{{ toastMsg }}</div>
  </div>
</template>

<script setup>
import { computed, ref, reactive, onMounted, onBeforeUnmount, nextTick, watch } from 'vue'
import { useRouter } from 'vue-router'
import { Terminal } from 'xterm'
import { FitAddon } from 'xterm-addon-fit'
import 'xterm/css/xterm.css'
import axios from 'axios'
import { API_BASE, WS_BASE } from '../runtimeBase'

const router = useRouter()

const sessions = ref([])
const termContainer = ref(null)
const activeSession = ref(null)
const termRefs = reactive({})
const termInstances = reactive({})
const fitAddons = reactive({})
const wsConnections = reactive({})
const toastMsg = ref('')
const systemInfo = ref({ hostname: 'localhost', os: 'Linux', arch: 'x86_64' })
const repoHref = 'https://github.com/user-no-found/FnRmux'
const clipboardOpen = ref(false)
const clipboardHistory = ref([])
const editingSession = ref(null)
const editingName = ref('')
const renameInputRefs = reactive({})

const dotClasses = ['online', 'blue', 'purple', 'orange']
const CLIPBOARD_HISTORY_LIMIT = 20

const authHeaders = () => {
  const token = sessionStorage.getItem('fnrmux_token')
  return token ? { headers: { Authorization: 'Bearer ' + token } } : { headers: {} }
}

const activeInfo = computed(() => sessions.value.find(s => s.session_id === activeSession.value))
const activePath = computed(() => activeInfo.value?.cwd || '')
const activeMeta = computed(() => {
  const size = activeInfo.value?.size
  const host = systemInfo.value.hostname || 'localhost'
  return `${host}${size ? ` · ${size.cols}x${size.rows}` : ''}`
})

const dotClass = (index) => dotClasses[index % dotClasses.length]

const sessionTitle = (session, index) => {
  if (session?.name) return session.name
  return index === 0 ? '本地终端' : `本地终端 ${index + 1}`
}

const showToast = (msg, duration = 2000) => {
  toastMsg.value = msg
  setTimeout(() => {
    if (toastMsg.value === msg) toastMsg.value = ''
  }, duration)
}

const setTermRef = (id, el) => {
  if (el) termRefs[id] = el
}

const setRenameInputRef = (id, el) => {
  if (el) renameInputRefs[id] = el
}

const sendTerminalInput = (data) => {
  const ws = wsConnections[activeSession.value]
  if (data && ws?.readyState === WebSocket.OPEN) {
    ws.send(JSON.stringify({ type: 'input', data }))
    return true
  }
  return false
}

const shellQuotePath = (path) => `'${String(path).replace(/'/g, `'\\''`)}'`

const makeClipboardId = () => {
  if (window.crypto?.randomUUID) return window.crypto.randomUUID()
  return `${Date.now()}-${Math.random().toString(16).slice(2)}`
}

const normalizeClipboardText = (text) => String(text || '').replace(/\r\n/g, '\n')

const loadClipboardHistory = async () => {
  try {
    const res = await axios.get(`${API_BASE}/api/clipboard`, authHeaders())
    if (res.data?.success && Array.isArray(res.data.data)) {
      clipboardHistory.value = res.data.data.slice(0, CLIPBOARD_HISTORY_LIMIT)
    }
  } catch (e) {
    clipboardHistory.value = []
  }
}

const sortSessionsByCreatedAt = (items) => [...items].sort((a, b) => {
  const ta = Date.parse(a.created_at || '') || 0
  const tb = Date.parse(b.created_at || '') || 0
  return ta - tb
})

const mergeSessionList = (incoming) => {
  const serverById = new Map(incoming.map(session => [session.session_id, session]))
  if (sessions.value.length === 0) return sortSessionsByCreatedAt(incoming)

  const merged = []
  for (const local of sessions.value) {
    const fresh = serverById.get(local.session_id)
    if (fresh) {
      merged.push(fresh)
      serverById.delete(local.session_id)
    }
  }

  const additions = sortSessionsByCreatedAt(Array.from(serverById.values()))
  return [...merged, ...additions]
}

const insertSessionAfterActive = (session) => {
  const activeIndex = sessions.value.findIndex(s => s.session_id === activeSession.value)
  if (activeIndex >= 0) {
    sessions.value.splice(activeIndex + 1, 0, session)
  } else {
    sessions.value.push(session)
  }
}

const toggleClipboard = async () => {
  const next = !clipboardOpen.value
  clipboardOpen.value = next
  if (next) await loadClipboardHistory()
}

const recordClipboardItem = async (item) => {
  const fallback = {
    ...item,
    id: item.id || makeClipboardId(),
    createdAt: item.createdAt || Date.now(),
  }
  const sameFallbackItem = (existing) => {
    if (fallback.kind === 'text') return existing.kind === 'text' && existing.text === fallback.text
    return existing.kind === 'image' && existing.path === fallback.path
  }
  clipboardHistory.value = [
    fallback,
    ...clipboardHistory.value.filter(existing => !sameFallbackItem(existing)),
  ].slice(0, CLIPBOARD_HISTORY_LIMIT)

  try {
    const payload = item.kind === 'image'
      ? { kind: 'image', path: item.path, contentType: item.contentType, size: item.size }
      : { kind: 'text', text: item.text }
    const res = await axios.post(`${API_BASE}/api/clipboard`, payload, authHeaders())
    if (res.data?.success && res.data.data) {
      const fresh = res.data.data
      const sameItem = (existing) => {
        if (fresh.kind === 'text') return existing.kind === 'text' && existing.text === fresh.text
        return existing.kind === 'image' && existing.path === fresh.path
      }
      clipboardHistory.value = [
        fresh,
        ...clipboardHistory.value.filter(existing => !sameItem(existing)),
      ].slice(0, CLIPBOARD_HISTORY_LIMIT)
    }
  } catch (e) {
    // 写入失败不影响粘贴主流程，本地历史已经先行更新。
  }
}

const clearClipboardHistory = async () => {
  try {
    await axios.delete(`${API_BASE}/api/clipboard`, authHeaders())
  } catch (e) {}
  clipboardHistory.value = []
}

const clipboardItemPreview = (item) => {
  if (item.kind === 'image') return item.path?.split('/').pop() || item.path || '图片文件'
  const text = normalizeClipboardText(item.text).replace(/\s+/g, ' ').trim()
  return text || '空文本'
}

const clipboardItemTitle = (item) => {
  if (item.kind === 'image') return item.path || '图片文件'
  return normalizeClipboardText(item.text)
}

const formatClipboardTime = (value) => {
  if (!value) return ''
  const date = new Date(value)
  return date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })
}

const pasteText = (text, { record = true } = {}) => {
  const normalized = normalizeClipboardText(text)
  if (!normalized) {
    showToast('剪切板没有文本')
    return false
  }
  if (!sendTerminalInput(normalized)) {
    showToast('没有可用终端')
    return false
  }
  if (record) recordClipboardItem({ kind: 'text', text: normalized })
  showToast('已粘贴文本')
  return true
}

const uploadClipboardImage = async (blob) => {
  const formData = new FormData()
  const ext = (blob.type || 'image/png').split('/')[1]?.replace('jpeg', 'jpg') || 'png'
  formData.append('image', blob, `paste.${ext}`)
  const res = await axios.post(`${API_BASE}/api/terminal/clipboard`, formData, authHeaders())
  return res.data?.data?.files?.[0]?.path || ''
}

const pasteImagePath = (path, { recordItem = null } = {}) => {
  if (!path) {
    showToast('图片路径无效')
    return false
  }
  if (!sendTerminalInput(`${shellQuotePath(path)} `)) {
    showToast('没有可用终端')
    return false
  }
  if (recordItem) recordClipboardItem(recordItem)
  showToast('图片路径已输入')
  return true
}

const pasteImageBlob = async (blob) => {
  if (!activeSession.value || !blob) return
  showToast('图片已保存，正在输入路径...')
  try {
    const path = await uploadClipboardImage(blob)
    if (path) {
      pasteImagePath(path, {
        recordItem: {
          kind: 'image',
          path,
          contentType: blob.type || 'image/png',
          size: blob.size || 0,
        },
      })
    } else {
      showToast('图片粘贴失败')
    }
  } catch (e) {
    showToast('图片粘贴失败: ' + (e.response?.data?.message || e.message || '未知错误'))
  }
}

const imageBlobFromPasteItems = (items) => {
  for (const item of Array.from(items || [])) {
    if (item.type?.startsWith('image/')) return item.getAsFile?.() || null
  }
  return null
}

const isEditableTarget = (target) => {
  const el = target instanceof Element ? target : null
  if (!el) return false
  if (el.closest('.xterm')) return false
  return Boolean(el.closest('input, textarea, select, [contenteditable="true"]'))
}

const handledPasteEvents = new WeakSet()
const handlePaste = (e) => {
  if (handledPasteEvents.has(e)) return
  handledPasteEvents.add(e)

  if (isEditableTarget(e.target)) return

  const imageBlob = imageBlobFromPasteItems(e.clipboardData?.items)
  if (imageBlob) {
    e.preventDefault()
    e.stopPropagation()
    pasteImageBlob(imageBlob)
    return
  }

  const text = e.clipboardData?.getData('text/plain')
  if (!text) return

  e.preventDefault()
  e.stopPropagation()
  pasteText(text)
}

const loadSystemInfo = async () => {
  try {
    const res = await axios.get(`${API_BASE}/api/system/info`, authHeaders())
    if (res.data.success) systemInfo.value = res.data.data
  } catch (e) {}
}

const loadSessions = async () => {
  try {
    const res = await axios.get(`${API_BASE}/api/sessions`, authHeaders())
    if (res.data.success) {
      sessions.value = mergeSessionList(res.data.data)
      if (sessions.value.length > 0 && !activeSession.value) {
        activeSession.value = sessions.value[0].session_id
      } else if (activeSession.value && !sessions.value.some(s => s.session_id === activeSession.value)) {
        activeSession.value = sessions.value[0]?.session_id || null
      }
    }
  } catch (e) {}
}

const initTerminal = async (id) => {
  const el = termRefs[id]
  if (!el || termInstances[id]) return

  const term = new Terminal({
    cursorBlink: true,
    fontSize: 14,
    fontFamily: 'Menlo, Monaco, "Courier New", monospace',
    lineHeight: 1.35,
    scrollback: 2000,
    allowProposedApi: true,
    convertEol: true,
    linkHandler: {
      activate: (_event, text) => openSafeUrl(text),
      allowNonHttpProtocols: false,
    },
    theme: {
      background: 'transparent',
      foreground: '#f4f7fb',
      cursor: '#f4f7fb',
      selectionBackground: 'rgba(85, 111, 132, 0.5)',
      black: '#17212a',
      red: '#ff5f6d',
      green: '#66e85f',
      yellow: '#ffb44c',
      blue: '#48a8ff',
      magenta: '#9a6dff',
      cyan: '#58d9ff',
      white: '#dce6ee',
      brightBlack: '#586676',
      brightRed: '#ff7a86',
      brightGreen: '#82f36f',
      brightYellow: '#ffc464',
      brightBlue: '#6cbaff',
      brightMagenta: '#ad88ff',
      brightCyan: '#80e6ff',
      brightWhite: '#ffffff',
    },
  })

  const fitAddon = new FitAddon()
  term.loadAddon(fitAddon)
  term.open(el)
  registerUrlLinks(term)

  termInstances[id] = term
  fitAddons[id] = fitAddon

  const token = sessionStorage.getItem('fnrmux_token') || ''
  const ws = new WebSocket(`${WS_BASE}/ws/terminal/${id}?token=${encodeURIComponent(token)}`)
  wsConnections[id] = ws

  ws.onopen = () => {
    setTimeout(() => {
      if (fitAddons[id]) {
        fitTerminal(id)
        if (ws.readyState === WebSocket.OPEN) {
          ws.send(JSON.stringify({ type: 'resize', cols: term.cols, rows: term.rows }))
        }
      }
      term.focus()
    }, 80)
  }

  ws.onmessage = (e) => {
    if (e.data === 'SESSION_NOT_FOUND' || e.data === 'SESSION_GONE') {
      term.write('\r\n\x1b[31m[会话已结束]\x1b[0m')
      return
    }
    term.write(e.data)
  }
  ws.onclose = () => term.write('\r\n\x1b[31m[连接已断开]\x1b[0m')

  term.onData((data) => {
    if (ws.readyState === WebSocket.OPEN) ws.send(JSON.stringify({ type: 'input', data }))
  })

  let resizeTimer = null
  term.onResize(({ cols, rows }) => {
    clearTimeout(resizeTimer)
    resizeTimer = setTimeout(() => {
      if (ws.readyState === WebSocket.OPEN) ws.send(JSON.stringify({ type: 'resize', cols, rows }))
    }, 220)
  })

  const termViewport = el.querySelector('.xterm-viewport') || el
  termViewport.addEventListener('contextmenu', async (e) => {
    e.preventDefault()
    if (term.hasSelection()) {
      await navigator.clipboard.writeText(term.getSelection())
      term.clearSelection()
      showToast('已复制')
    } else {
      await pasteClipboard()
    }
  })

  term.attachCustomKeyEventHandler((e) => {
    if (e.type === 'keydown' && e.ctrlKey && e.key === 'Enter') {
      if (ws.readyState === WebSocket.OPEN) ws.send(JSON.stringify({ type: 'input', data: '\n' }))
      return false
    }
    return true
  })
}

const switchSession = async (id) => {
  activeSession.value = id
  await nextTick()
  if (!termInstances[id]) {
    await initTerminal(id)
  } else {
    setTimeout(() => {
      fitTerminal(id)
      termInstances[id]?.focus()
    }, 20)
  }
}

const createNewSession = async () => {
  try {
    let cols = 80
    let rows = 24
    if (termContainer.value) {
      const term = new Terminal({ fontSize: 14, fontFamily: 'Menlo, Monaco, "Courier New", monospace' })
      const fitAddon = new FitAddon()
      term.loadAddon(fitAddon)
      const dummy = document.createElement('div')
      dummy.style.visibility = 'hidden'
      dummy.style.position = 'absolute'
      dummy.style.width = '100%'
      dummy.style.height = '100%'
      termContainer.value.appendChild(dummy)
      term.open(dummy)
      fitAddon.fit()
      cols = term.cols
      rows = term.rows
      termContainer.value.removeChild(dummy)
      term.dispose()
    }

    const res = await axios.post(`${API_BASE}/api/sessions`, { type: 'local', cols, rows }, authHeaders())
    if (res.data.success) {
      const newSess = res.data.data
      insertSessionAfterActive(newSess)
      activeSession.value = newSess.session_id
      await nextTick()
      await initTerminal(newSess.session_id)
    }
  } catch (e) {
    showToast('创建失败: ' + (e.response?.data?.message || e.message))
  }
}

const closeSession = async (id) => {
  try {
    await axios.delete(`${API_BASE}/api/sessions/${id}`, authHeaders())
  } catch (e) {}
  if (wsConnections[id]) {
    wsConnections[id].close()
    delete wsConnections[id]
  }
  if (termInstances[id]) {
    termInstances[id].dispose()
    delete termInstances[id]
  }
  if (fitAddons[id]) delete fitAddons[id]
  sessions.value = sessions.value.filter(s => s.session_id !== id)
  if (activeSession.value === id) activeSession.value = sessions.value[0]?.session_id || null
}

const beginRename = async (session) => {
  if (!session) return
  editingSession.value = session.session_id
  editingName.value = session.name || '本地终端'
  await nextTick()
  const input = renameInputRefs[session.session_id]
  input?.focus()
  input?.select()
}

const cancelRename = () => {
  editingSession.value = null
  editingName.value = ''
}

const commitRename = async (session) => {
  if (!session || editingSession.value !== session.session_id) return
  const currentName = session.name || '本地终端'
  const name = editingName.value.trim()
  editingSession.value = null
  editingName.value = ''

  if (!name || name === currentName) return

  try {
    const res = await axios.patch(`${API_BASE}/api/sessions/${session.session_id}/name`, { name }, authHeaders())
    if (res.data.success) {
      const updated = res.data.data
      const index = sessions.value.findIndex(s => s.session_id === session.session_id)
      if (index >= 0) sessions.value[index] = updated
      showToast('名称已更新')
    }
  } catch (e) {
    showToast(e.response?.data?.message || '重命名失败')
  }
}

const openSafeUrl = (raw) => {
  const url = String(raw || '').replace(/[),.;:!?]+$/g, '')
  if (!/^https?:\/\//i.test(url)) return
  window.open(url, '_blank', 'noopener,noreferrer')
}

const registerUrlLinks = (term) => {
  const urlPattern = /\bhttps?:\/\/[^\s<>"'`]+/gi
  term.registerLinkProvider({
    provideLinks(bufferLineNumber, callback) {
      const line = term.buffer.active.getLine(bufferLineNumber - 1)
      const text = line?.translateToString(true) || ''
      const links = []
      for (const match of text.matchAll(urlPattern)) {
        const raw = match[0]
        const url = raw.replace(/[),.;:!?]+$/g, '')
        if (!url) continue
        const start = (match.index || 0) + 1
        const end = start + url.length - 1
        links.push({
          range: {
            start: { x: start, y: bufferLineNumber },
            end: { x: end, y: bufferLineNumber },
          },
          text: url,
          decorations: { pointerCursor: true, underline: true },
          activate: (_event, value) => openSafeUrl(value),
        })
      }
      callback(links.length ? links : undefined)
    },
  })
}

const fitTerminal = (id) => {
  const term = termInstances[id]
  const fitAddon = fitAddons[id]
  if (!term || !fitAddon) return
  requestAnimationFrame(() => {
    try {
      fitAddon.fit()
      term.refresh(0, term.rows - 1)
    } catch (e) {}
  })
}

const copySelection = async () => {
  const term = termInstances[activeSession.value]
  if (!term || !term.hasSelection()) {
    showToast('没有选中文本')
    return
  }
  await navigator.clipboard.writeText(term.getSelection())
  term.clearSelection()
  showToast('已复制')
}

const pasteClipboard = async () => {
  try {
    if (navigator.clipboard?.read) {
      const items = await navigator.clipboard.read()
      for (const item of items) {
        const imageType = item.types.find(type => type.startsWith('image/'))
        if (imageType) {
          await pasteImageBlob(await item.getType(imageType))
          return
        }
      }
    }

    if (navigator.clipboard?.readText) {
      const text = await navigator.clipboard.readText()
      pasteText(text)
    }
  } catch (e) {
    showToast('粘贴失败: ' + (e.response?.data?.message || e.message || '请使用 Ctrl+V 粘贴'))
  }
}

const pasteClipboardHistoryItem = (item) => {
  if (item.kind === 'image') {
    if (pasteImagePath(item.path, { recordItem: item })) clipboardOpen.value = false
    return
  }

  if (pasteText(item.text, { record: false })) {
    recordClipboardItem(item)
    clipboardOpen.value = false
  }
}

const openInBrowser = () => {
  const route = router.resolve('/terminal')
  const url = new URL(route.href, window.location.origin).toString()
  const opened = window.open(url, '_blank', 'noopener,noreferrer')
  if (!opened) showToast('浏览器阻止了新窗口')
}

let globalResizeTimer = null
let sessionRefreshTimer = null
const handleResize = () => {
  clearTimeout(globalResizeTimer)
  globalResizeTimer = setTimeout(() => {
    if (activeSession.value && fitAddons[activeSession.value]) fitTerminal(activeSession.value)
  }, 240)
}

onMounted(async () => {
  loadClipboardHistory()
  await Promise.all([loadSessions(), loadSystemInfo()])
  window.addEventListener('resize', handleResize)
  window.addEventListener('paste', handlePaste, true)
  document.addEventListener('paste', handlePaste, true)
  sessionRefreshTimer = setInterval(loadSessions, 3000)
  if (activeSession.value) {
    await nextTick()
    await initTerminal(activeSession.value)
  }
})

onBeforeUnmount(() => {
  window.removeEventListener('resize', handleResize)
  window.removeEventListener('paste', handlePaste, true)
  document.removeEventListener('paste', handlePaste, true)
  clearInterval(sessionRefreshTimer)
  Object.values(wsConnections).forEach(ws => ws.close())
  Object.values(termInstances).forEach(t => t.dispose())
})

watch(activeSession, async (newVal) => {
  if (newVal) {
    await nextTick()
    if (!termInstances[newVal]) await initTerminal(newVal)
    else setTimeout(() => {
      fitTerminal(newVal)
      termInstances[newVal]?.focus()
    }, 20)
  }
})
</script>

<style scoped>
.terminal-shell {
  --bg: #071019;
  --panel: #0b151d;
  --panel-2: #101b24;
  --line: rgba(214, 231, 242, 0.12);
  --line-strong: rgba(214, 231, 242, 0.18);
  --text: #f4f7fb;
  --muted: #aab5c1;
  --dim: #657381;
  --green: #59df55;
  --blue: #3da7f7;
  --purple: #8d67f3;
  --orange: #ffae42;
  min-height: 100vh;
  display: grid;
  grid-template-columns: minmax(0, 1fr);
  color: var(--text);
  background:
    radial-gradient(circle at 82% 14%, rgba(55, 102, 130, 0.2), transparent 34%),
    linear-gradient(145deg, #040a10 0%, #0b1620 48%, #071019 100%);
  overflow: hidden;
  font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", system-ui, sans-serif;
}

button {
  font: inherit;
}

.icon-action,
.tab-rename,
.tab-close {
  border: 0;
  color: var(--text);
  background: transparent;
  cursor: pointer;
}

.icon-action:hover,
.tab-rename:hover,
.tab-close:hover,
.tool-btn:hover {
  background: rgba(255, 255, 255, 0.07);
}

.status-dot {
  width: 12px;
  height: 12px;
  flex: 0 0 auto;
  border-radius: 50%;
  display: inline-block;
  background: var(--dim);
  box-shadow: 0 0 18px currentColor;
}

.status-dot.online {
  color: var(--green);
  background: var(--green);
}

.status-dot.blue {
  color: var(--blue);
  background: var(--blue);
}

.status-dot.purple {
  color: var(--purple);
  background: var(--purple);
}

.status-dot.orange {
  color: var(--orange);
  background: var(--orange);
}

.terminal-panel {
  min-width: 0;
  min-height: 100vh;
  display: grid;
  grid-template-rows: 56px minmax(0, 1fr) 48px;
  background: rgba(6, 13, 19, 0.52);
}

.terminal-tabs {
  min-width: 0;
  display: flex;
  align-items: stretch;
  justify-content: space-between;
  border-bottom: 1px solid var(--line);
  background: rgba(7, 16, 23, 0.66);
}

.tabs-scroll {
  min-width: 0;
  display: flex;
  align-items: stretch;
  overflow-x: auto;
  overflow-y: hidden;
}

.terminal-tab {
  min-width: 0;
  max-width: 260px;
  height: 56px;
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 0 16px;
  border: 0;
  border-right: 1px solid rgba(214, 231, 242, 0.08);
  color: #edf3f8;
  background: transparent;
  cursor: pointer;
  font-size: 16px;
}

.terminal-tab.active {
  background: rgba(255, 255, 255, 0.07);
}

.terminal-tab .tab-title {
  min-width: 0;
  max-width: 150px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.tab-name-input {
  width: min(150px, 36vw);
  min-width: 84px;
  height: 32px;
  padding: 0 8px;
  border: 1px solid rgba(72, 168, 255, 0.55);
  border-radius: 6px;
  outline: none;
  color: #edf3f8;
  background: rgba(4, 10, 16, 0.82);
  font: inherit;
  font-size: 16px;
}

.terminal-tab.add-tab {
  width: 52px;
  min-width: 52px;
  justify-content: center;
  font-size: 24px;
  color: #d6dee7;
}

.tab-rename,
.tab-close {
  flex: 0 0 28px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  padding: 0;
  border-radius: 6px;
  color: var(--muted);
  font-size: 18px;
  line-height: 1;
}

.tab-close {
  font-size: 22px;
}

.toolbar {
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 0 16px;
}

.tool-btn,
.icon-action {
  height: 42px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  border: 0;
  border-radius: 8px;
  color: #d6dee7;
  background: transparent;
  cursor: pointer;
  font-size: 16px;
}

.tool-btn {
  padding: 0 8px;
}

.tool-icon {
  color: #d6dee7;
  font-size: 19px;
}

.clipboard-menu {
  position: relative;
  display: inline-flex;
  align-items: center;
}

.clipboard-trigger {
  position: relative;
}

.history-count {
  min-width: 20px;
  height: 20px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  padding: 0 6px;
  border-radius: 999px;
  color: #06100c;
  background: var(--green);
  font-size: 12px;
  font-weight: 800;
  line-height: 1;
}

.clipboard-popover {
  position: absolute;
  top: calc(100% + 8px);
  right: 0;
  z-index: 20;
  width: min(420px, calc(100vw - 24px));
  max-height: min(520px, calc(100vh - 94px));
  display: grid;
  grid-template-rows: auto auto minmax(0, 1fr);
  border: 1px solid var(--line-strong);
  border-radius: 8px;
  background: rgba(8, 17, 25, 0.98);
  box-shadow: 0 24px 60px rgba(0, 0, 0, 0.42);
  overflow: hidden;
}

.clipboard-head {
  height: 44px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 14px;
  border-bottom: 1px solid var(--line);
  color: #edf3f8;
  font-weight: 700;
}

.clipboard-close {
  width: 30px;
  height: 30px;
  border: 0;
  border-radius: 6px;
  color: var(--muted);
  background: transparent;
  cursor: pointer;
  font-size: 22px;
  line-height: 1;
}

.clipboard-close:hover {
  color: var(--text);
  background: rgba(255, 255, 255, 0.08);
}

.clipboard-actions {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  gap: 8px;
  padding: 10px;
  border-bottom: 1px solid var(--line);
}

.clipboard-actions button {
  height: 34px;
  border: 1px solid rgba(214, 231, 242, 0.14);
  border-radius: 7px;
  color: #dce6ee;
  background: rgba(255, 255, 255, 0.05);
  cursor: pointer;
  font-size: 13px;
}

.clipboard-actions button:disabled {
  color: var(--dim);
  cursor: default;
  opacity: 0.55;
}

.clipboard-actions button:not(:disabled):hover {
  background: rgba(255, 255, 255, 0.1);
}

.clipboard-empty {
  padding: 28px 16px;
  color: var(--muted);
  text-align: center;
  font-size: 14px;
}

.clipboard-list {
  min-height: 0;
  overflow: auto;
  padding: 6px;
}

.clipboard-item {
  width: 100%;
  min-height: 48px;
  display: grid;
  grid-template-columns: 44px minmax(0, 1fr) auto;
  align-items: center;
  gap: 10px;
  padding: 8px;
  border: 0;
  border-radius: 7px;
  color: #e8f0f6;
  background: transparent;
  cursor: pointer;
  text-align: left;
}

.clipboard-item:hover {
  background: rgba(255, 255, 255, 0.07);
}

.clipboard-kind {
  height: 24px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border-radius: 6px;
  font-size: 12px;
  font-weight: 800;
}

.clipboard-kind.text {
  color: #d8edf6;
  background: rgba(72, 168, 255, 0.18);
}

.clipboard-kind.image {
  color: #ecf7df;
  background: rgba(89, 223, 85, 0.18);
}

.clipboard-preview {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: #edf3f8;
  font-size: 14px;
}

.clipboard-time {
  color: var(--dim);
  font-family: Menlo, Monaco, "Courier New", monospace;
  font-size: 12px;
}

.icon-action {
  width: 42px;
  font-size: 24px;
}

.terminal-viewport {
  position: relative;
  min-width: 0;
  min-height: 0;
  padding: 24px 28px;
  background:
    radial-gradient(circle at 50% 0, rgba(42, 82, 104, 0.12), transparent 38%),
    rgba(2, 8, 13, 0.58);
}

.term-wrapper {
  position: absolute;
  inset: 24px 28px;
  display: block;
  visibility: hidden;
  pointer-events: none;
}

.term-wrapper.active {
  visibility: visible;
  pointer-events: auto;
}

:deep(.xterm) {
  height: 100%;
  padding: 0;
}

:deep(.xterm-viewport) {
  background: transparent !important;
  scrollbar-width: thin;
  scrollbar-color: rgba(255, 255, 255, 0.18) transparent;
}

:deep(.xterm-viewport)::-webkit-scrollbar {
  width: 6px;
  height: 6px;
}

:deep(.xterm-viewport)::-webkit-scrollbar-track {
  background: transparent;
}

:deep(.xterm-viewport)::-webkit-scrollbar-thumb {
  background: rgba(255, 255, 255, 0.14);
  border-radius: 3px;
  transition: background 0.2s ease;
}

:deep(.xterm-viewport):hover::-webkit-scrollbar-thumb {
  background: rgba(255, 255, 255, 0.28);
}

:deep(.xterm-viewport)::-webkit-scrollbar-thumb:hover {
  background: rgba(255, 255, 255, 0.42);
}

:deep(.xterm-viewport)::-webkit-scrollbar-button {
  display: none;
}

:deep(.xterm-viewport)::-webkit-scrollbar-corner {
  background: transparent;
}

:deep(.xterm-screen) {
  min-height: 100%;
}

.empty-state {
  height: 100%;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 14px;
  color: var(--muted);
  font-family: Menlo, Monaco, "Courier New", monospace;
}

.empty-title {
  color: #f4f7fb;
  font-size: 24px;
}

.empty-line {
  font-size: 14px;
}

.primary-command {
  margin-top: 8px;
  padding: 11px 16px;
  border: 0;
  border-radius: 8px;
  color: #041008;
  background: var(--green);
  cursor: pointer;
  font-weight: 800;
}

.statusbar {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto minmax(0, 1fr);
  align-items: center;
  column-gap: 20px;
  padding: 0 28px;
  border-top: 1px solid var(--line);
  color: #d6dee7;
  background: rgba(7, 16, 23, 0.7);
  font-size: 16px;
}

.status-left,
.status-center,
.status-right {
  display: flex;
  align-items: center;
  gap: 14px;
  min-width: 0;
}

.status-left {
  justify-self: start;
  min-width: 0;
  overflow: hidden;
}

.status-center {
  justify-content: center;
  color: #a9bac8;
  font-size: 14px;
  justify-self: center;
  min-width: 0;
  white-space: nowrap;
}

.repo-label {
  color: #a9bac8;
}

.status-center a {
  color: #79d96b;
  text-decoration: none;
}

.status-center a:hover {
  text-decoration: underline;
}

.status-right {
  justify-self: end;
  min-width: 0;
  color: #c2cad3;
  font-family: Menlo, Monaco, "Courier New", monospace;
}

.status-path {
  color: #9fbdce;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  max-width: min(44vw, 520px);
}

.paste-toast {
  position: fixed;
  left: 50%;
  bottom: 34px;
  transform: translateX(-50%);
  z-index: 10;
  padding: 10px 16px;
  border: 1px solid var(--line-strong);
  border-radius: 8px;
  color: #f4f7fb;
  background: rgba(9, 18, 26, 0.94);
  box-shadow: 0 18px 44px rgba(0, 0, 0, 0.32);
}

@media (max-width: 980px) {
  .terminal-tabs {
    flex-direction: column;
    height: auto;
  }

  .terminal-panel {
    grid-template-rows: auto minmax(0, 1fr) 44px;
  }

  .toolbar {
    height: 50px;
    padding: 0 12px;
    border-top: 1px solid rgba(214, 231, 242, 0.08);
    overflow-x: auto;
  }

  .terminal-tab {
    height: 48px;
    font-size: 15px;
  }

  .tool-btn > span:not(.tool-icon):not(.history-count) {
    display: none;
  }

  .clipboard-popover {
    position: fixed;
    top: 104px;
    right: 12px;
    width: calc(100vw - 24px);
    max-height: calc(100vh - 160px);
  }

  .statusbar {
    padding: 0 14px;
    font-size: 12px;
    column-gap: 10px;
  }

  .status-right span:first-child {
    display: none;
  }

  .status-center {
    display: none;
  }

  .status-path {
    max-width: 42vw;
  }
}

@media (max-width: 640px) {
  .terminal-viewport {
    padding: 14px;
  }

  .term-wrapper {
    inset: 14px;
  }
}
</style>
