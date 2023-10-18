import { CmdRunner } from "./cmd_runner.js";
import { DropDown } from "./desktop.js";
import { applist, desktop } from "./global.js";
window.onload = () => {
    const body = document.querySelector("body");
    body.appendChild(desktop.root);
    console.log("onload");
    const applications = new DropDown({
        title: "Applications",
        items: applist.map(app => {
            return {
                title: app.name,
                onClick: () => {
                    app.start();
                }
            };
        })
    });
    desktop.toolbar.addToolbarButton(applications);
    let cmd_runner;
    document.addEventListener("keydown", function (event) {
        if (event.ctrlKey && event.key === "q" || event.key === "Q") {
            event.preventDefault();
            if (!cmd_runner) {
                cmd_runner = new CmdRunner();
                body.appendChild(cmd_runner.root);
            }
        }
        else if (event.key === "Escape" && cmd_runner) {
            cmd_runner.destroy();
            cmd_runner = undefined;
        }
    });
    window.addEventListener("mousedown", function (event) {
        if (cmd_runner) {
            cmd_runner.destroy();
            cmd_runner = undefined;
        }
    });
};
if (location.protocol === "https:") {
    if ("serviceWorker" in navigator) {
        window.addEventListener("load", function () {
            navigator.serviceWorker.register("/PuppyOS/service-worker.js").then(function (registration) {
                // Registration was successful
                console.log("ServiceWorker registration successful with scope: ", registration.scope);
            }, function (err) {
                // registration failed :(
                console.log("ServiceWorker registration failed: ", err);
            });
        });
    }
}
