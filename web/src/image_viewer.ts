import { Win } from "./desktop.js";

export class ImageViewer extends Win {
    public constructor(args: {
        src: string
    }) {
        super({
            title: "Image Viewer"
        })

        const img = document.createElement("img")
        img.src = args.src
        img.style.width = "100%"
        this.root.style.overflow = "hidden"
        img.draggable = false
        this.content.appendChild(img)
    }
}