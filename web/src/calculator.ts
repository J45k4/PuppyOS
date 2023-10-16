import { Win } from "./desktop.js";

class CalcResult {
    public root: HTMLDivElement

    public constructor() {
        this.root = document.createElement("div")
        this.root.style.height = "80px"
        this.root.style.overflow = "auto"
        this.root.contentEditable = "true"
    }  
}

type CalcExpr = "+" | "-" | "*" | "/" | "="

class NumberPad {
    public root: HTMLDivElement

    public constructor(args: {
        onNumberClick: (num: number) => void
        onActionClick: (action: CalcExpr) => void
    }) {
        this.root = document.createElement("div")
        // this.root.style.height = "100%"
        this.root.style.display = "flex"
        this.root.style.flexDirection = "row"
        this.root.onmousedown = (e) => {
            e.stopPropagation()
            e.preventDefault()
        }

        const middle = document.createElement("div")
        middle.style.display = "flex"
        middle.style.flexDirection = "column"
        middle.style.flexGrow = "1"
        this.root.appendChild(middle)

        for (let i = 0; i < 3; i++) {
            const row = document.createElement("div")
            row.style.display = "flex"
            row.style.flexGrow = "1"
            row.style.flexDirection = "row"
            row.style.justifyContent = "space-evenly"
            row.style.alignItems = "center"
            row.style.width = "100%"
            row.style.flexGrow = "1"
            middle.appendChild(row)

            for (let j = 0; j < 3; j++) {
                const num = i * 3 + j + 1

                const btn = document.createElement("button")
                btn.innerHTML = num.toString()
                // btn.style.width = "50px"
                // btn.style.height = "50px"
                // btn.style.borderRadius = "25px"
                btn.style.height = "100%"
                btn.style.border = "none"
                btn.style.backgroundColor = "lightgray"
                btn.style.cursor = "pointer"
                btn.style.outline = "none"
                btn.style.flexGrow = "1"
                btn.onmouseover = () => {
                    btn.style.backgroundColor = "gray"
                }
                btn.onmouseout = () => {
                    btn.style.backgroundColor = "lightgray"
                }
                btn.onclick = () => {
                    args.onNumberClick(num)
                }
                row.appendChild(btn)
            }
        }

        const row = document.createElement("div")
        row.style.display = "flex"
        row.style.flexGrow = "1"
        row.style.flexDirection = "row"
        middle.appendChild(row)

        const zeroBtn = document.createElement("button")
        zeroBtn.innerHTML = "0"
        zeroBtn.style.outline = "none"
        zeroBtn.style.border = "none"
        zeroBtn.style.flexGrow = "1"
        zeroBtn.style.cursor = "pointer"
        row.appendChild(zeroBtn)

        const decimalBtn = document.createElement("button")
        decimalBtn.innerHTML = "."
        decimalBtn.style.outline = "none"
        decimalBtn.style.border = "none"
        decimalBtn.style.flexGrow = "1"
        decimalBtn.style.cursor = "pointer"
        row.appendChild(decimalBtn)

        const equalsBtn = document.createElement("button")
        equalsBtn.innerHTML = "="
        equalsBtn.style.outline = "none"
        equalsBtn.style.border = "none"
        equalsBtn.style.flexGrow = "1"
        equalsBtn.style.cursor = "pointer"
        equalsBtn.onclick = () => {
            args.onActionClick("=")
        }
        row.appendChild(equalsBtn)

        const right = document.createElement("div")
        right.style.flexGrow = "1"
        right.style.maxWidth = "50px",
        right.style.display = "flex"
        right.style.flexDirection = "column"
        this.root.appendChild(right)

        const timesBtn = document.createElement("button")
        timesBtn.style.flexGrow = "1"
        timesBtn.innerHTML = "*"
        timesBtn.style.width = "100%"
        timesBtn.style.outline = "none"
        timesBtn.style.border = "none"
        timesBtn.style.flexGrow = "1"
        timesBtn.style.cursor = "pointer"
        timesBtn.onclick = () => {
            args.onActionClick("*")
        }
        right.appendChild(timesBtn)

        const divideBtn = document.createElement("button")
        divideBtn.style.flexGrow = "1"
        divideBtn.innerHTML = "/"
        divideBtn.style.width = "100%"
        divideBtn.style.outline = "none"
        divideBtn.style.border = "none"
        divideBtn.style.flexGrow = "1"
        divideBtn.style.cursor = "pointer"
        divideBtn.onclick = () => {
            args.onActionClick("/")
        }
        right.appendChild(divideBtn)

        const minusBtn = document.createElement("button")
        minusBtn.style.flexGrow = "1"
        minusBtn.innerHTML = "-"
        minusBtn.style.width = "100%"
        minusBtn.style.outline = "none"
        minusBtn.style.border = "none"
        minusBtn.style.flexGrow = "1"
        minusBtn.style.cursor = "pointer"
        minusBtn.onclick = () => {
            args.onActionClick("-")
        }
        right.appendChild(minusBtn)

        const plusBtn = document.createElement("button")
        plusBtn.style.flexGrow = "1"
        plusBtn.innerHTML = "+"
        plusBtn.style.width = "100%"
        plusBtn.style.outline = "none"
        plusBtn.style.border = "none"
        plusBtn.style.flexGrow = "1"
        plusBtn.style.cursor = "pointer"
        plusBtn.onclick = () => {
            args.onActionClick("+")
        }
        right.appendChild(plusBtn)
    }
}

export class CalculatorApp extends Win {
    public constructor() {
        super({
            title: "Calculator",
            minHeight: 200,
            minWidth: 200,
        })

        let expr_string = ""

        this.content.style.display = "flex"
        this.content.style.flexDirection = "column"

        const res = new CalcResult()
        this.content.appendChild(res.root)
        res.root.innerHTML = expr_string.toString()

        const numberPad = new NumberPad({
            onNumberClick: (n) => {
                console.log("clicked", n)
                expr_string += n.toString()
                res.root.innerHTML = expr_string.toString()
            },
            onActionClick: (action) => {
                if (action === "=") {
                    console.log("evaluating")
                    try {
                        const res = math.evaluate(expr_string)
                        console.log("res", res)
                        expr_string += " = " + res.toString()
                    } catch (e) {
                        console.error(e)
                        expr_string = "invalid expression"
                    }

                    res.root.innerHTML = expr_string.toString()

                    return
                }

                expr_string += action.toString()
                res.root.innerHTML = expr_string.toString()
            }
        })
        numberPad.root.style.flexGrow = "1"
        this.content.appendChild(numberPad.root)
    }
}