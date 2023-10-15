import { Win } from "./desktop.js";
class CalcResult {
}
class NumberPad {
    root;
    constructor(args) {
        this.root = document.createElement("div");
        this.root.style.height = "100%";
        this.root.style.display = "flex";
        this.root.style.flexDirection = "row";
        this.root.onmousedown = (e) => {
            e.stopPropagation();
            e.preventDefault();
        };
        const middle = document.createElement("div");
        middle.style.display = "flex";
        middle.style.flexDirection = "column";
        middle.style.flexGrow = "1";
        this.root.appendChild(middle);
        for (let i = 0; i < 4; i++) {
            const row = document.createElement("div");
            row.style.display = "flex";
            row.style.flexGrow = "1";
            row.style.flexDirection = "row";
            row.style.justifyContent = "space-evenly";
            row.style.alignItems = "center";
            row.style.width = "100%";
            row.style.flexGrow = "1";
            middle.appendChild(row);
            for (let j = 0; j < 3; j++) {
                const btn = document.createElement("button");
                btn.innerHTML = (i * 3 + j + 1).toString();
                // btn.style.width = "50px"
                // btn.style.height = "50px"
                // btn.style.borderRadius = "25px"
                btn.style.height = "100%";
                btn.style.border = "none";
                btn.style.backgroundColor = "lightgray";
                btn.style.cursor = "pointer";
                btn.style.outline = "none";
                btn.style.flexGrow = "1";
                btn.onmouseover = () => {
                    btn.style.backgroundColor = "gray";
                };
                btn.onmouseout = () => {
                    btn.style.backgroundColor = "lightgray";
                };
                row.appendChild(btn);
            }
        }
        const row = document.createElement("div");
        row.style.display = "flex";
        row.style.flexGrow = "1";
        row.style.flexDirection = "row";
        middle.appendChild(row);
        const zeroBtn = document.createElement("button");
        zeroBtn.innerHTML = "0";
        zeroBtn.style.outline = "none";
        zeroBtn.style.border = "none";
        zeroBtn.style.flexGrow = "1";
        zeroBtn.style.cursor = "pointer";
        row.appendChild(zeroBtn);
        const decimalBtn = document.createElement("button");
        decimalBtn.innerHTML = ".";
        decimalBtn.style.outline = "none";
        decimalBtn.style.border = "none";
        decimalBtn.style.flexGrow = "1";
        decimalBtn.style.cursor = "pointer";
        row.appendChild(decimalBtn);
        const equalsBtn = document.createElement("button");
        equalsBtn.innerHTML = "=";
        equalsBtn.style.outline = "none";
        equalsBtn.style.border = "none";
        equalsBtn.style.flexGrow = "1";
        equalsBtn.style.cursor = "pointer";
        row.appendChild(equalsBtn);
        const right = document.createElement("div");
        right.style.flexGrow = "1";
        right.style.maxWidth = "50px",
            right.style.display = "flex";
        right.style.flexDirection = "column";
        this.root.appendChild(right);
        const timesBtn = document.createElement("button");
        timesBtn.style.flexGrow = "1";
        timesBtn.innerHTML = "*";
        timesBtn.style.width = "100%";
        timesBtn.style.outline = "none";
        timesBtn.style.border = "none";
        timesBtn.style.flexGrow = "1";
        timesBtn.style.cursor = "pointer";
        right.appendChild(timesBtn);
        const divideBtn = document.createElement("button");
        divideBtn.style.flexGrow = "1";
        divideBtn.innerHTML = "/";
        divideBtn.style.width = "100%";
        divideBtn.style.outline = "none";
        divideBtn.style.border = "none";
        divideBtn.style.flexGrow = "1";
        divideBtn.style.cursor = "pointer";
        right.appendChild(divideBtn);
        const minusBtn = document.createElement("button");
        minusBtn.style.flexGrow = "1";
        minusBtn.innerHTML = "-";
        minusBtn.style.width = "100%";
        minusBtn.style.outline = "none";
        minusBtn.style.border = "none";
        minusBtn.style.flexGrow = "1";
        minusBtn.style.cursor = "pointer";
        right.appendChild(minusBtn);
    }
}
export class CalculatorApp extends Win {
    constructor() {
        super({
            title: "Calculator"
        });
        const numberPad = new NumberPad({
            onClick: (num) => {
                console.log("clicked", num);
            }
        });
        this.content.appendChild(numberPad.root);
    }
}
