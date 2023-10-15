import { Win } from "./desktop.js";

export class EditorApp extends Win {
    public constructor() {
        super({
            title: "Editor"
        })

        const textarea = document.createElement("textarea")
        textarea.style.width = "100%"
        textarea.style.height = "100%"
        textarea.style.resize = "none"
        textarea.style.border = "none"
        textarea.style.outline = "none"

        this.content.appendChild(textarea)
    }
}