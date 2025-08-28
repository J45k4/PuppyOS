import Foundation
import UserNotifications

enum NotificationManager {
    static func requestAuthorization() {
        UNUserNotificationCenter.current().requestAuthorization(options: [.alert, .sound, .badge]) { granted, error in
            if let error { print("Notification auth error: \(error)") }
            print("Notifications granted: \(granted)")
        }
    }
    
    static func scheduleTimerNotification(id: String, title: String, fireDate: Date) {
        let content = UNMutableNotificationContent()
        content.title = "Timer Finished"
        content.body = title.isEmpty ? "Your timer is up." : title
        content.sound = .default
        let timeInterval = max(1, fireDate.timeIntervalSinceNow)
        let trigger = UNTimeIntervalNotificationTrigger(timeInterval: timeInterval, repeats: false)
        let request = UNNotificationRequest(identifier: id, content: content, trigger: trigger)
        UNUserNotificationCenter.current().add(request) { error in
            if let error { print("Failed to schedule: \(error)") }
        }
    }
    
    static func cancelNotification(id: String) {
        UNUserNotificationCenter.current().removePendingNotificationRequests(withIdentifiers: [id])
    }
}

