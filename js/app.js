import { AccountantApp } from "./accountant.js";
import { CalculatorApp } from "./calculator.js";
import { Calendar } from "./calendar.js";
import { PuppyChat } from "./chat.js";
import { Desktop, DropDown } from "./desktop.js";
import { EditorApp } from "./editor.js";
import { Email } from "./email.js";
import { ImageViewer } from "./image_viewer.js";
import { SheetApp } from "./sheet.js";
import { TerminalApp } from "./terminal.js";
import { Timer } from "./timer.js";
window.onload = () => {
    const body = document.querySelector("body");
    console.log("onload");
    const desktop = new Desktop(body);
    const applications = new DropDown({
        title: "Applications",
        items: [
            {
                title: "Accountant",
                onClick: () => {
                    const app = new AccountantApp();
                    desktop.root.appendChild(app.root);
                }
            },
            {
                title: "Image Viewer",
                onClick: () => {
                    const app = new ImageViewer({
                        src: "/PuppyOS/puppy_pillow.png"
                    });
                    desktop.root.appendChild(app.root);
                }
            },
            {
                title: "Calculator",
                onClick: () => {
                    const app = new CalculatorApp();
                    desktop.root.appendChild(app.root);
                }
            },
            {
                title: "Terminal",
                onClick: () => {
                    const app = new TerminalApp();
                    desktop.root.appendChild(app.root);
                }
            },
            {
                title: "Editor",
                onClick: () => {
                    const app = new EditorApp();
                    desktop.root.appendChild(app.root);
                }
            },
            {
                title: "Calendar",
                onClick: () => {
                    const app = new Calendar();
                    desktop.root.appendChild(app.root);
                }
            },
            {
                title: "PuppyChat",
                onClick: () => {
                    const app = new PuppyChat();
                    desktop.root.appendChild(app.root);
                }
            },
            {
                title: "Email",
                onClick: () => {
                    const app = new Email();
                    desktop.root.appendChild(app.root);
                }
            },
            {
                title: "Sheet",
                onClick: () => {
                    const sheep = new SheetApp();
                    desktop.root.appendChild(sheep.root);
                }
            },
            {
                title: "Timer",
                onClick: () => {
                    const app = new Timer({
                        time: 20
                    });
                    desktop.root.appendChild(app.root);
                }
            }
        ]
    });
    desktop.toolbar.addToolbarButton(applications);
    // const icon = new DesktopIcon({
    //     src: "/PuppyOS/account_manager.jpeg",
    //     onClick: () => {
    //         const app = new AccountantApp()
    //         desktop.root.appendChild(app.root)
    //     }
    // })
    // desktop.root.appendChild(icon.root)
    // const icon2 = new DesktopIcon({
    //     src: "/PuppyOS/puppy_pillow.png",
    //     onClick: () => {
    //         const app = new ImageViewer({
    //             src: "/puppy_pillow.png"
    //         })
    //         desktop.root.appendChild(app.root)
    //     }
    // })
    // desktop.root.appendChild(icon2.root)
};
