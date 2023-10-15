import { Win } from "./desktop.js";
export class Timer extends Win {
    constructor() {
        super({
            title: "Timer",
            minHeight: 200,
            minWidth: 200,
            maxHeight: 200,
            maxWidth: 200
        });
        const container = document.createElement("div");
        this.content.appendChild(container);
        container.innerHTML = `
        <div class="timerContainer">
            <div class="currentTimerTime">
                55s
            </div>
            <div class="newTime">
                <input 
                    type="text"
                    placeholder="Enter time"
                />
            </div>
            <div class="timerControls">
                <button class="timer_cntr_button">Reset</button>
                <button class="timer_cntr_button">Start</button>
            </div>
        </div>
        `;
        const timerTime = container.querySelector(".currentTimerTime");
        timerTime.onmousedown = (e) => {
            e.stopPropagation();
        };
        const newTime = container.querySelector(".newTime");
        newTime.onmousedown = (e) => {
            e.stopPropagation();
        };
        const timerControls = container.querySelector(".timerControls");
        timerControls.onmousedown = (e) => {
            e.stopPropagation();
        };
    }
}
