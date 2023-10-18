import { AccountantApp } from "./accountant.js"
import { CalculatorApp } from "./calculator.js"
import { Calendar } from "./calendar.js"
import { PuppyChat } from "./chat.js"
import { Desktop } from "./desktop.js"
import { EditorApp } from "./editor.js"
import { Email } from "./email.js"
import { ImageViewer } from "./image_viewer.js"
import { SheetApp } from "./sheet.js"
import { TerminalApp } from "./terminal.js"
import { Timer } from "./timer.js"

export const desktop = new Desktop()

export const applist = [
    {
        name: "Accountant",
        start: () => {
            const app = new AccountantApp()
            desktop.root.appendChild(app.root)
        }
    },
    {
        name: "Image Viewer",
        start: () => {
            const app = new ImageViewer({
                src: "/PuppyOS/puppy_pillow.png"
            })
            desktop.root.appendChild(app.root)
        }
    },
    {
        name: "Calculator",
        start: () => {
            const app = new CalculatorApp()
            desktop.root.appendChild(app.root)
        }
    },
    {
        name: "Terminal",
        start: () => {
            const app = new TerminalApp()
            desktop.root.appendChild(app.root)
        }
    },
    {
        name: "Editor",
        start: () => {
            const app = new EditorApp()
            desktop.root.appendChild(app.root)
        }
    },
    {
        name: "Calendar",
        start: () => {
            const app = new Calendar()
            desktop.root.appendChild(app.root)
        }
    },
    {
        name: "PuppyChat",
        start: () => {
            const app = new PuppyChat()
            desktop.root.appendChild(app.root)
        }
    },
    {
        name: "Email",
        start: () => {
            const app = new Email()
            desktop.root.appendChild(app.root)
        }
    },
    {
        name: "Sheet",
        start: () => {
            const sheep = new SheetApp()
            desktop.root.appendChild(sheep.root)
        }
    },
    {
        name: "Timer",
        start: () => {
            const app = new Timer({
                time: 20
            })
            desktop.root.appendChild(app.root)
        }
    }
]