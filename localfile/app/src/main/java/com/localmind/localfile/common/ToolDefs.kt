package com.localmind.localfile.common

import org.json.JSONArray
import org.json.JSONObject

/**
 * OpenAI 兼容的工具定义（function calling）。
 */
object ToolDefs {

    /**
     * 生成文档工具：AI 通过 function calling 调用，生成 docx/pptx/pdf/xlsx 文件。
     * AI 根据用户要求选 format，把内容结构化传入，聊天端解析后生成文件并分享。
     */
    fun buildGenerateDocTool(): JSONObject = JSONObject().apply {
        put("type", "function")
        put("function", JSONObject().apply {
            put("name", "generate_doc")
            put("description",
                "生成文档文件。当用户要求生成/创建/制作 Word 文档、PPT 演示文稿、PDF、Excel 表格时调用。")
            put("parameters", JSONObject().apply {
                put("type", "object")
                put("properties", JSONObject().apply {
                    put("format", JSONObject().apply {
                        put("type", "string")
                        put("enum", JSONArray().apply {
                            put("docx"); put("pptx"); put("pdf"); put("xlsx")
                        })
                        put("description", "文件格式：docx=Word文档, pptx=PPT演示文稿, pdf=PDF, xlsx=Excel表格。用户说 Word/文档→docx，PPT/演示→pptx，Excel/表格→xlsx，PDF→pdf")
                    })
                    put("title", JSONObject().apply {
                        put("type", "string")
                        put("description", "文档标题")
                    })
                    put("paragraphs", JSONObject().apply {
                        put("type", "array")
                        put("items", JSONObject().apply { put("type", "string") })
                        put("description", "正文段落列表（docx/pdf 用）")
                    })
                    put("bullets", JSONObject().apply {
                        put("type", "array")
                        put("items", JSONObject().apply { put("type", "string") })
                        put("description", "要点列表（可选，docx/pdf 用）")
                    })
                    put("slides", JSONObject().apply {
                        put("type", "array")
                        put("items", JSONObject().apply {
                            put("type", "object")
                            put("properties", JSONObject().apply {
                                put("title", JSONObject().apply { put("type", "string"); put("description", "页标题") })
                                put("bullets", JSONObject().apply {
                                    put("type", "array")
                                    put("items", JSONObject().apply { put("type", "string") })
                                    put("description", "本页要点")
                                })
                            })
                        })
                        put("description", "PPT 页面列表（pptx 用）：每页含 title 和 bullets")
                    })
                    put("sheetName", JSONObject().apply {
                        put("type", "string")
                        put("description", "Excel 工作表名（xlsx 用）")
                    })
                    put("headers", JSONObject().apply {
                        put("type", "array")
                        put("items", JSONObject().apply { put("type", "string") })
                        put("description", "Excel 表头（xlsx 用）")
                    })
                    put("rows", JSONObject().apply {
                        put("type", "array")
                        put("items", JSONObject().apply {
                            put("type", "array")
                            put("items", JSONObject().apply { put("type", "string") })
                        })
                        put("description", "Excel 数据行（xlsx 用，每行一个数组）")
                    })
                })
                put("required", JSONArray().apply { put("format"); put("title") })
            })
        })
    }

    /** 工具列表（JSONArray） */
    fun generateDocTools(): JSONArray = JSONArray().apply {
        put(buildGenerateDocTool())
    }
}
