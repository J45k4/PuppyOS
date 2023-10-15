import { AccountantApp } from "./accountant.js";
import { CalculatorApp } from "./calculator.js";
import { Desktop, DesktopIcon, DropDown } from "./desktop.js";
import { ImageViewer } from "./image_viewer.js";
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
                        src: "/puppy_pillow.png"
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
            }
        ]
    });
    desktop.toolbar.addToolbarButton(applications);
    const icon = new DesktopIcon({
        src: "/account_manager.jpeg",
        onClick: () => {
            const app = new AccountantApp();
            desktop.root.appendChild(app.root);
        }
    });
    desktop.root.appendChild(icon.root);
    const icon2 = new DesktopIcon({
        src: "/puppy_pillow.png",
        onClick: () => {
            const app = new ImageViewer({
                src: "/puppy_pillow.png"
            });
            desktop.root.appendChild(app.root);
        }
    });
    desktop.root.appendChild(icon2.root);
};
