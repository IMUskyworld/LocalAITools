package com.localmind.localfile.ui.chat

import android.app.Application
import android.net.Uri
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import com.localmind.localfile.chat.ChatEngine
import com.localmind.localfile.chat.ChatRepository
import com.localmind.localfile.chat.ChatStreamEvent
import com.localmind.localfile.common.ChatMessage
import com.localmind.localfile.common.Logger
import com.localmind.localfile.files.FileInfo
import com.localmind.localfile.files.FileProcessor
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import java.io.File

data class ChatUiState(
    val sessions: List<ChatRepository.Session> = emptyList(),
    val currentSessionId: String? = null,
    val messages: List<ChatMessage> = emptyList(),
    val inputText: String = "",
    val isStreaming: Boolean = false,
    val streamingContent: String = "",
    val isLoadingSessions: Boolean = false,
    val speedText: String = "",
    val error: String? = null,
    val attachedFile: FileInfo? = null,
    val attachedFileContent: String? = null,
    val generatedFile: File? = null,
    val thinkingSteps: List<ChatStreamEvent.ThinkingStep> = emptyList()
)

class ChatViewModel(application: Application) : AndroidViewModel(application) {

    private val chatRepo = ChatRepository(getApplication())
    private val chatEngine = ChatEngine(getApplication())
    private val fileProcessor = FileProcessor(getApplication())

    private val _state = MutableStateFlow(ChatUiState())
    val state: StateFlow<ChatUiState> = _state.asStateFlow()

    private var streamJob: Job? = null

    init {
        loadSessions()
    }

    fun loadSessions() {
        viewModelScope.launch {
            _state.update { it.copy(isLoadingSessions = true) }
            try {
                val sessions = chatRepo.listSessions()
                _state.update { it.copy(sessions = sessions, isLoadingSessions = false) }
                if (sessions.isNotEmpty() && _state.value.currentSessionId == null) {
                    selectSession(sessions.first().id)
                }
            } catch (e: Exception) {
                Logger.e("loadSessions failed", e)
                _state.update { it.copy(isLoadingSessions = false) }
            }
        }
    }

    fun createSession(title: String = "") {
        viewModelScope.launch {
            try {
                val session = chatRepo.createSession(title)
                _state.update {
                    it.copy(
                        sessions = listOf(session) + it.sessions,
                        currentSessionId = session.id,
                        messages = emptyList(),
                        inputText = ""
                    )
                }
            } catch (e: Exception) {
                Logger.e("createSession failed", e)
                _state.update { it.copy(error = "操作失败：${e.message}") }
            }
        }
    }

    fun selectSession(sessionId: String) {
        viewModelScope.launch {
            _state.update { it.copy(currentSessionId = sessionId, messages = emptyList(), error = null) }
            try {
                val msgs = chatRepo.listMessages(sessionId)
                _state.update { it.copy(messages = msgs.map { m -> ChatMessage(m.role, m.content) }) }
            } catch (e: Exception) {
                Logger.e("selectSession failed", e)
            }
        }
    }

    fun deleteSession(sessionId: String) {
        viewModelScope.launch {
            try {
                chatRepo.deleteSession(sessionId)
                val sessions = _state.value.sessions.filter { it.id != sessionId }
                val newCurrentId = if (_state.value.currentSessionId == sessionId)
                    sessions.firstOrNull()?.id else _state.value.currentSessionId
                _state.update { it.copy(sessions = sessions, currentSessionId = newCurrentId, messages = emptyList()) }
                newCurrentId?.let { selectSession(it) }
            } catch (e: Exception) {
                Logger.e("deleteSession failed", e)
            }
        }
    }

    fun updateInput(text: String) {
        _state.update { it.copy(inputText = text) }
    }

    /**
     * 附加文件：读取文件内容，作为对话上下文。
     */
    fun attachFile(uri: Uri) {
        viewModelScope.launch {
            try {
                val fileInfo = fileProcessor.getFileInfo(uri)
                val content = fileProcessor.readFileContent(uri)
                _state.update { it.copy(attachedFile = fileInfo, attachedFileContent = content, error = null) }
            } catch (e: Exception) {
                Logger.e("attachFile failed", e)
                _state.update { it.copy(error = "读取文件失败：${e.message}") }
            }
        }
    }

    fun removeAttachment() {
        _state.update { it.copy(attachedFile = null, attachedFileContent = null) }
    }

    fun sendMessage() {
        val input = _state.value.inputText.trim()
        if (input.isEmpty()) return

        var sessionId = _state.value.currentSessionId
        if (sessionId == null) {
            val title = if (input.length > 30) input.take(30) else input
            viewModelScope.launch {
                try {
                    val session = chatRepo.createSession(title)
                    _state.update {
                        it.copy(sessions = listOf(session) + it.sessions, currentSessionId = session.id,
                            messages = emptyList(), inputText = "")
                    }
                    doSendMessage(session.id, input)
                } catch (e: Exception) {
                    _state.update { it.copy(error = "操作失败：${e.message}") }
                }
            }
            return
        }

        doSendMessage(sessionId, input)
    }

    private fun doSendMessage(sessionId: String, input: String) {
        val messagesBeforeUser = _state.value.messages
        val attachedName = _state.value.attachedFile?.name
        val attachedContent = _state.value.attachedFileContent

        // 若附加了文件，把文件内容作为 system 上下文注入，让 AI 基于内容回答
        val messagesForAI: List<ChatMessage> = if (attachedContent != null) {
            listOf(ChatMessage("system",
                "用户上传了文件「${attachedName ?: "附件"}」，内容如下，请基于文件内容回答用户的问题：\n\n$attachedContent")) +
                messagesBeforeUser
        } else {
            messagesBeforeUser
        }

        _state.update { it.copy(inputText = "", error = null, streamingContent = "") }

        streamJob = viewModelScope.launch {
            try {
                // Persist user message locally
                chatRepo.appendMessage(sessionId, "user", input, modelLabel())
                _state.update { it.copy(messages = it.messages + ChatMessage("user", input)) }

                // 发送后清除附件（一次对话消费掉）
                _state.update { it.copy(attachedFile = null, attachedFileContent = null) }

                // Stream AI response（含 Agent 循环：可能生成文件）
                chatEngine.sendMessage(messagesForAI, input, sessionId).collect { event ->
                    when (event) {
                        is ChatStreamEvent.StreamStart -> _state.update { it.copy(isStreaming = true, thinkingSteps = emptyList()) }
                        is ChatStreamEvent.Token -> _state.update { it.copy(streamingContent = it.streamingContent + event.text) }
                        is ChatStreamEvent.Speed -> _state.update { it.copy(speedText = event.text) }
                        is ChatStreamEvent.GeneratedFile -> _state.update { it.copy(generatedFile = event.file) }
                        is ChatStreamEvent.ThinkingStep -> _state.update { it.copy(thinkingSteps = it.thinkingSteps + event) }
                        is ChatStreamEvent.StreamEnd -> {
                            val fullContent = _state.value.streamingContent
                            val usage = event.usage
                            if (fullContent.isNotEmpty()) {
                                chatRepo.appendMessage(
                                    sessionId, "assistant", fullContent, modelLabel(),
                                    tokenCount = usage?.totalTokens ?: 0,
                                )
                            }
                            _state.update {
                                it.copy(
                                    messages = it.messages + ChatMessage("assistant", fullContent, usage = usage),
                                    isStreaming = false, streamingContent = "", speedText = ""
                                )
                            }
                        }
                        is ChatStreamEvent.Error -> _state.update { it.copy(isStreaming = false, error = event.message) }
                        is ChatStreamEvent.ToolCallStarted,
                        is ChatStreamEvent.ToolCall,
                        is ChatStreamEvent.ToolCallEnd -> { /* 工具调用过程提示，暂不显示 */ }
                    }
                }
            } catch (e: Exception) {
                Logger.e("sendMessage error", e)
                _state.update { it.copy(isStreaming = false, error = "操作失败：${e.message}") }
            }
        }
    }

    fun stopGeneration() {
        chatEngine.stopGeneration()
        streamJob?.cancel()
        val partial = _state.value.streamingContent
        if (partial.isNotEmpty()) {
            _state.update { it.copy(messages = it.messages + ChatMessage("assistant", partial), isStreaming = false, streamingContent = "") }
        } else {
            _state.update { it.copy(isStreaming = false) }
        }
    }

    fun clearError() { _state.update { it.copy(error = null) } }

    fun clearGeneratedFile() { _state.update { it.copy(generatedFile = null) } }

    private fun modelLabel() = "DeepSeek V4 Flash"
}
