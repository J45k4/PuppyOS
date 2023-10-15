import { Win } from "./desktop.js";
const diffTime = (timeout_date) => {
    const now = new Date();
    if (timeout_date <= now)
        return 0;
    let diff = (timeout_date.getTime() - now.getTime()) / 1000; // get difference in seconds
    return diff;
};
const createTimeString = (diff) => {
    if (diff <= 0)
        return 'Time is up!';
    const days = Math.floor(diff / (24 * 60 * 60));
    diff -= days * (24 * 60 * 60);
    const hours = Math.floor(diff / (60 * 60));
    diff -= hours * (60 * 60);
    const minutes = Math.floor(diff / 60);
    diff -= minutes * 60;
    const seconds = Math.floor(diff);
    let timeStr = '';
    if (days > 0)
        timeStr += `${days} day${days !== 1 ? 's' : ''}, `;
    if (hours > 0)
        timeStr += `${hours} hour${hours !== 1 ? 's' : ''}, `;
    if (minutes > 0)
        timeStr += `${minutes} minute${minutes !== 1 ? 's' : ''}, `;
    if (seconds > 0)
        timeStr += `${seconds} second${seconds !== 1 ? 's' : ''}`;
    // Remove trailing comma and space
    timeStr = timeStr.replace(/, $/, '');
    return timeStr || 'Less than a second left!';
};
export class Timer extends Win {
    running = false;
    started = false;
    time = 0;
    timeleft = 0;
    timeout_date;
    constructor(args) {
        super({
            title: "Timer",
            minHeight: 200,
            minWidth: 200,
            maxHeight: 200,
            maxWidth: 200
        });
        const alert_sound = new Audio("/PuppyOS/radar_alert.mp3");
        const container = document.createElement("div");
        this.content.appendChild(container);
        this.time = args.time || 0;
        this.timeleft = args.time || 0;
        this.timeout_date = new Date(Date.now() + this.timeleft * 1000);
        container.innerHTML = `
        <div class="timerContainer">
            <div class="currentTimerTime">
                ${createTimeString(diffTime(this.timeout_date))}
            </div>
            <div class="newTime">
                <input 
                    type="text"
                    placeholder="Enter time"
                />
            </div>
            <div class="timerControls">
                <button class="timer_cntr_button bt1">Reset</button>
                <button class="timer_cntr_button bt2">Start</button>
            </div>
        </div>
        `;
        const timerCurrentTime = container.querySelector(".currentTimerTime");
        timerCurrentTime.onmousedown = (e) => {
            e.stopPropagation();
        };
        const btn1 = container.querySelector(".bt1");
        const btn2 = container.querySelector(".bt2");
        btn1.onclick = () => {
            this.timeleft = this.time;
            this.timeout_date = new Date(Date.now() + this.timeleft * 1000);
            this.running = false;
            this.started = false;
            btn2.innerHTML = "Start";
            timerCurrentTime.innerHTML = createTimeString(diffTime(this.timeout_date));
        };
        btn2.onclick = () => {
            if (!this.started) {
                this.started = true;
                this.running = true;
                this.timeout_date = new Date(Date.now() + this.timeleft * 1000);
                btn2.innerHTML = "Pause";
            }
            else if (this.running) {
                this.running = false;
                this.timeleft = diffTime(this.timeout_date);
                btn2.innerHTML = "Continue";
            }
            else {
                this.running = true;
                this.timeout_date = new Date(Date.now() + this.timeleft * 1000);
                btn2.innerHTML = "Pause";
            }
        };
        const newTime = container.querySelector(".newTime");
        newTime.onmousedown = (e) => {
            e.stopPropagation();
        };
        const timerControls = container.querySelector(".timerControls");
        timerControls.onmousedown = (e) => {
            e.stopPropagation();
        };
        setInterval(() => {
            if (!this.running) {
                return;
            }
            const diff = diffTime(this.timeout_date);
            timerCurrentTime.innerHTML = createTimeString(diff);
            if (diff === 0) {
                this.running = false;
                alert_sound.play();
            }
        }, 1000);
    }
}
