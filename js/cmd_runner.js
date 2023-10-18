import { applist } from "./global.js";
export class CmdRunner {
    root;
    input;
    results;
    currentSelection;
    keydownListener;
    constructor() {
        this.root = document.createElement("div");
        this.root.style.position = "absolute";
        this.root.style.top = "0px";
        this.root.style.left = "0px";
        this.root.style.width = "100%";
        this.root.style.height = "100%";
        this.root.style.display = "flex";
        this.root.style.flexDirection = "column";
        this.root.style.alignItems = "center";
        this.root.style.justifyContent = "center";
        const content = document.createElement("div");
        content.onmousedown = (event) => {
            event.stopPropagation();
        };
        this.root.appendChild(content);
        this.input = document.createElement("input");
        this.input.type = "text";
        this.input.autofocus = true;
        this.input.style.fontSize = "30px";
        this.input.style.width = "100%";
        this.input.onkeydown = (event) => {
            if (event.key === "Enter") {
                this.handleCmd(this.input.value);
                this.input.value = "";
            }
        };
        this.input.oninput = e => {
            this.handleCmdChange(this.input.value);
        };
        content.appendChild(this.input);
        this.results = document.createElement("div");
        this.results.style.height = "400px";
        this.results.style.width = "400px";
        this.results.style.backgroundColor = "white";
        this.results.style.border = "1px solid black";
        content.appendChild(this.results);
        this.keydownListener = this.onKeydown.bind(this);
        document.addEventListener("keydown", this.keydownListener);
    }
    onKeydown(e) {
        if (e.key === "ArrowUp") {
            if (this.currentSelection == null) {
                return;
            }
            this.unsetCurrentSelection();
            if (this.currentSelection === 0) {
                this.currentSelection = undefined;
            }
            else {
                this.currentSelection -= 1;
            }
            this.setCurrentSelection();
        }
        if (e.key === "ArrowDown") {
            if (this.currentSelection == null) {
                this.currentSelection = 0;
                this.setCurrentSelection();
                return;
            }
            if (this.currentSelection === this.results.children.length - 1) {
                return;
            }
            this.unsetCurrentSelection();
            this.currentSelection += 1;
            this.setCurrentSelection();
        }
        if (e.key === "Enter") {
            if (this.currentSelection == null) {
                return;
            }
            const current = this.results.children[this.currentSelection];
            this.handleCmd(current.innerText);
        }
    }
    setCurrentSelection() {
        if (this.currentSelection == null) {
            return;
        }
        const current = this.results.children[this.currentSelection];
        current.style.border = "2px solid black";
        current.focus();
    }
    unsetCurrentSelection() {
        if (this.currentSelection == null) {
            return;
        }
        const current = this.results.children[this.currentSelection];
        current.style.border = "none";
    }
    handleCmdChange(cmd) {
        const results = this.results;
        results.innerHTML = "";
        const cmdLower = cmd.toLowerCase();
        if (cmdLower === "") {
            return;
        }
        const matches = applist.filter(app => {
            return app.name.toLowerCase().includes(cmdLower);
        });
        for (const match of matches) {
            const result = document.createElement("div");
            result.style.padding = "5px";
            result.style.cursor = "pointer";
            result.style.borderBottom = "1px solid black";
            result.innerText = match.name;
            result.onclick = () => {
                this.handleCmd(match.name);
            };
            results.appendChild(result);
        }
    }
    handleCmd(cmd) {
        const app = applist.find(app => {
            return app.name.toLowerCase() === cmd.toLowerCase();
        });
        if (app) {
            app.start();
            return;
        }
    }
    destroy() {
        document.removeEventListener("keydown", this.keydownListener);
        this.root.remove();
    }
}
