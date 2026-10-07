import { createApp } from 'vue'
import App from './App.vue'
import DownloadNotice from './components/DownloadNotice.vue'
import { noticeApi } from './services/notices'
import './styles/tokens.css'
import './styles/components.css'

const isNotice = noticeApi.windowLabel() === 'download-notice'
if (isNotice) document.body.style.minWidth = '0'
createApp(isNotice ? DownloadNotice : App).mount('#app')
