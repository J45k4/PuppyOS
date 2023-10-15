import { Win } from "./desktop.js";
export class TerminalApp extends Win {
    constructor() {
        super({
            title: "Terminal"
        });
        const textarea = document.createElement("textarea");
        textarea.style.width = "100%";
        textarea.style.height = "100%";
        textarea.style.resize = "none";
        textarea.style.border = "none";
        textarea.style.outline = "none";
        this.content.appendChild(textarea);
    }
}
