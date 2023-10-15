export class DesktopIcon {
    public root: HTMLImageElement

    public constructor(args: {
        src?: string
        onClick?: () => void
    }) {
        this.root = document.createElement("img")
        this.root.style.cursor = "pointer"
        this.root.src = args.src
        this.root.style.width = 50 + "px"
        this.root.style.height = 50 + "px"

        this.root.onclick = args.onClick
    }
}

export class Desktop {
    public root: HTMLElement
    public toolbar: Toolbar
    private content: HTMLDivElement

    public constructor(root: HTMLElement) {
        this.root = root
        this.root.style.backgroundRepeat = "no-repeat"
        this.root.style.backgroundSize = "cover"
        this.content = document.createElement("div")
        this.root.appendChild(this.content)
        this.toolbar = new Toolbar()
        this.root.appendChild(this.toolbar.root)
        this.root.style.backgroundImage = `url("/PuppyOS/logo.jpeg")`
    }

    public addWind(win: Win) {
        this.root.appendChild(win.root)
    }

    public addIcon(icon: DesktopIcon) {
        this.root.appendChild(icon.root)
    }
}

export class Toolbar {
    public root: HTMLElement

    public constructor() {
        this.root = document.createElement("div")
        this.root.style.width = "100%"
        this.root.style.height = "50px"
        this.root.style.backgroundColor = "white"
        this.root.style.display = "flex"
    }

    public addToolbarButton(btn: ToolbarButton) {
        btn.root.style.margin = "5px"
        btn.root.style.padding = "5px"
        this.root.appendChild(btn.root)
    }
}

export class DropDown {
    public root: HTMLDivElement
    public itemsDiv: HTMLDivElement

    public constructor(args: {
        title: string
        items: {
            title: string
            onClick: () => void
        }[]
    }) {
        this.root = document.createElement("div")
        this.root.innerHTML = args.title
        // this.root.style.maxHeight = "20px"
        // this.root.style.overflowY = "hidden"
        this.root.style.cursor = "pointer"
        this.root.style.zIndex = "100"

        this.root.onmouseover = () => {
            this.root.style.maxHeight = null
            this.itemsDiv.style.display = "block"
        }
        this.root.onmouseout = () => {
            // this.root.style.maxHeight = "20px"
            this.itemsDiv.style.display = "none"
        }

        this.itemsDiv = document.createElement("div")
        this.itemsDiv.style.display = "none"
        this.itemsDiv.style.backgroundColor = "white"

        for (const item of args.items) {
            const itemDiv = document.createElement("div")
            itemDiv.style.border = "5px"
            itemDiv.innerHTML = item.title
            itemDiv.className = "dropDownItem"
            itemDiv.onclick = item.onClick
            this.itemsDiv.appendChild(itemDiv)
        }

        this.root.appendChild(this.itemsDiv)
    }
}

export class Win {
    public root: HTMLDivElement
    public content: HTMLDivElement

    private title: string
    private moving: boolean
    private dragStartX: number
    private dragStartY: number
    private dragClientX: number
    private dragClientY: number
    private xResizing: boolean
    private yResizing: boolean
    private width: number
    private height: number

    public constructor(args: {
        title?: string
        minHeight?: number
        minWidth?: number
        maxHeight?: number
        maxWidth?: number
    }) {
        this.title = args.title || "Window"
        this.root = document.createElement("div")
        this.root.style.position = "absolute"
        this.root.style.zIndex = "100"

        this.height = 100
        this.width = 100

        const winToolbar = document.createElement("div")
        winToolbar.style.border = "1px solid black"
        winToolbar.style.backgroundColor = "white"
        winToolbar.style.display = "flex"
        winToolbar.style.flexDirection = "row"
        const toolbarTitle = document.createElement("div")
        toolbarTitle.style.flexGrow = "1"
        toolbarTitle.innerHTML = this.title
        const toolbarControls = document.createElement("div")
        const closeBtn = document.createElement("button")
        closeBtn.innerHTML = "X"
        closeBtn.style.margin = "5px"
        closeBtn.onclick = (e) => {
            e.preventDefault()
            e.stopPropagation()
            this.root.remove()
        }
        toolbarControls.appendChild(closeBtn)

        winToolbar.appendChild(toolbarTitle)
        winToolbar.appendChild(toolbarControls)

        this.root.appendChild(winToolbar)

        this.content = document.createElement("div")
        this.content.style.border = "1px solid black"
        this.content.style.height = this.height + "px"
        this.content.style.width = this.width + "px"
        this.content.style.minHeight = args.minHeight ? args.minHeight + "px" : undefined
        this.content.style.minWidth = args.minWidth ? args.minWidth + "px" : undefined
        this.content.style.maxHeight = args.maxHeight ? args.maxHeight + "px" : undefined
        this.content.style.maxWidth = args.maxWidth ? args.maxWidth + "px" : undefined
        this.content.style.backgroundColor = "white"

        const rightResize = document.createElement("div")
        rightResize.style.width = "5px"
        rightResize.style.marginLeft = "-5px"
        rightResize.style.cursor = "ew-resize"
        rightResize.onmousedown = (ev: DragEvent) => {
            if (ev.button === 0) {
                ev.stopPropagation()
                console.log("bottomResize.onmousedown")
                this.xResizing = true
                this.dragClientX = ev.clientX
                this.dragClientY = ev.clientY
                this.width = this.content.clientWidth
            }
        }
        rightResize.onmouseup = (ev: DragEvent) => {
            ev.stopPropagation()
            console.log("bottomResize.onmouseup")
            this.xResizing = false
        }

        const middle = document.createElement("div")
        middle.style.display = "flex"
        middle.style.flexDirection = "row"
        middle.appendChild(this.content)
        middle.appendChild(rightResize)
        this.root.appendChild(middle)

        const bottomResize = document.createElement("div")
        bottomResize.style.height = "5px"
        bottomResize.style.marginTop = "-5px"
        bottomResize.style.flexGrow = "1"
        bottomResize.style.cursor = "ns-resize"
        this.root.appendChild(bottomResize)
        bottomResize.onmousedown = (ev: DragEvent) => {
            if (ev.button === 0) {
                ev.stopPropagation()
                console.log("bottomResize.onmousedown")
                this.yResizing = true
                this.dragClientX = ev.clientX
                this.dragClientY = ev.clientY
                this.height = this.content.clientHeight
            }
        }
        bottomResize.onmouseup = (ev: DragEvent) => {
            ev.stopPropagation()
            console.log("bottomResize.onmouseup")
            this.yResizing = false
        }

        const rightDownResize = document.createElement("div")
        rightDownResize.style.width = "5px"
        rightDownResize.style.height = "5px"
        rightDownResize.style.marginLeft = "-5px"
        rightDownResize.style.marginTop = "-5px"
        rightDownResize.style.cursor = "nwse-resize"
        rightDownResize.onmousedown = (ev: DragEvent) => {
            if (ev.button === 0) {
                ev.stopPropagation()
                console.log("rightDownResize.onmousedown")
                this.xResizing = true
                this.yResizing = true
                this.dragClientX = ev.clientX
                this.dragClientY = ev.clientY
                this.height = this.content.clientHeight
                this.width = this.content.clientWidth
            }
        }
        rightDownResize.onmouseup = (ev: DragEvent) => {
            ev.stopPropagation()
            console.log("rightDownResize.onmouseup")

            this.xResizing = false
            this.yResizing = false
        }

        const bottom = document.createElement("div")
        bottom.style.display = "flex"
        bottom.style.flexDirection = "row"
        bottom.appendChild(bottomResize)
        bottom.appendChild(rightDownResize)
        this.root.appendChild(bottom)

        this.root.onmousedown = (ev: DragEvent) => {
            console.log("onDragStart")
            this.moving = true
            console.log("offsetX: " + ev.offsetX)
            console.log("offsetY: " + ev.offsetY)
            console.log("clientX: " + ev.clientX)
            console.log("clientY: " + ev.clientY)
            this.dragStartX = ev.offsetX
            this.dragStartY = ev.offsetY
        }

        window.addEventListener("mouseup", (ev: DragEvent) => {
            console.log("onDragEnd")
            this.moving = false
            this.xResizing = false
            this.yResizing = false
        })

        window.addEventListener("mousemove", (ev: DragEvent) => {
            if (this.moving) {
                this.root.style.left = (ev.clientX - this.dragStartX) + "px"
                this.root.style.top = (ev.clientY - this.dragStartY - 20) + "px"
            }

            if (this.xResizing) {
                const width = this.width + ev.clientX - this.dragClientX
                this.content.style.width = width + "px"
            }

            if (this.yResizing) {
                const height = this.height + ev.clientY - this.dragClientY
                this.content.style.height = height + "px"
            }
        })
    }
}

export class ToolbarButton {
    public root: HTMLDivElement

    public constructor() {
        this.root = document.createElement("div")
        this.root.innerHTML = "ToolbarButton"
    }
}
