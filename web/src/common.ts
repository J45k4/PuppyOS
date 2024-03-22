
export class ObservableVariable {
    private value: any
    private listeners: ((value: any) => void)[] = []

    constructor(value: any) {
        this.value = value
    }

    get() {
        return this.value
    }

    set(value: any) {
        this.value = value
        this.listeners.forEach(listener => listener(value))
    }

    onChange(listener: (value: any) => void) {
        this.listeners.push(listener)
    }
} 