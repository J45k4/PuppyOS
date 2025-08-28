import Foundation
import CoreData

@objc(TimeEntryEntity)
class TimeEntryEntity: NSManagedObject {
    @NSManaged var id: UUID
    @NSManaged var title: String
    @NSManaged var start: Date
    @NSManaged var end: Date
    
    @nonobjc class func fetchRequest() -> NSFetchRequest<TimeEntryEntity> {
        NSFetchRequest<TimeEntryEntity>(entityName: "TimeEntryEntity")
    }
    
    var duration: TimeInterval { end.timeIntervalSince(start) }
}

// Make Core Data entity identifiable for SwiftUI lists
extension TimeEntryEntity: Identifiable {}

@objc(TimerEntity)
class TimerEntity: NSManagedObject {
    @NSManaged var id: UUID
    @NSManaged var title: String
    @NSManaged var end: Date
    @NSManaged var isRunning: Bool
    @NSManaged var notificationId: String?
    
    @nonobjc class func fetchRequest() -> NSFetchRequest<TimerEntity> {
        NSFetchRequest<TimerEntity>(entityName: "TimerEntity")
    }
}

extension TimerEntity: Identifiable {}

struct PersistenceController {
    static let shared = PersistenceController()
    let container: NSPersistentContainer
    
    init(inMemory: Bool = false) {
        let model = Self.makeModel()
        container = NSPersistentContainer(name: "TimeModel", managedObjectModel: model)
        if inMemory {
            let description = NSPersistentStoreDescription()
            description.type = NSInMemoryStoreType
            container.persistentStoreDescriptions = [description]
        }
        container.loadPersistentStores { _, error in
            if let error = error as NSError? {
                fatalError("Unresolved error: \(error), \(error.userInfo)")
            }
        }
        container.viewContext.mergePolicy = NSMergeByPropertyObjectTrumpMergePolicy
        container.viewContext.automaticallyMergesChangesFromParent = true
    }
    
    private static func makeModel() -> NSManagedObjectModel {
        let model = NSManagedObjectModel()
        
        let timeEntry = NSEntityDescription()
        timeEntry.name = "TimeEntryEntity"
        timeEntry.managedObjectClassName = NSStringFromClass(TimeEntryEntity.self)
        
        let id = NSAttributeDescription()
        id.name = "id"
        id.attributeType = .UUIDAttributeType
        id.isOptional = false
        
        let title = NSAttributeDescription()
        title.name = "title"
        title.attributeType = .stringAttributeType
        title.isOptional = false
        title.defaultValue = ""
        
        let start = NSAttributeDescription()
        start.name = "start"
        start.attributeType = .dateAttributeType
        start.isOptional = false
        
        let end = NSAttributeDescription()
        end.name = "end"
        end.attributeType = .dateAttributeType
        end.isOptional = false
        
        timeEntry.properties = [id, title, start, end]

        // TimerEntity
        let timerEntity = NSEntityDescription()
        timerEntity.name = "TimerEntity"
        timerEntity.managedObjectClassName = NSStringFromClass(TimerEntity.self)

        let tId = NSAttributeDescription()
        tId.name = "id"
        tId.attributeType = .UUIDAttributeType
        tId.isOptional = false

        let tTitle = NSAttributeDescription()
        tTitle.name = "title"
        tTitle.attributeType = .stringAttributeType
        tTitle.isOptional = false
        tTitle.defaultValue = ""

        let tEnd = NSAttributeDescription()
        tEnd.name = "end"
        tEnd.attributeType = .dateAttributeType
        tEnd.isOptional = false

        let tRunning = NSAttributeDescription()
        tRunning.name = "isRunning"
        tRunning.attributeType = .booleanAttributeType
        tRunning.isOptional = false
        tRunning.defaultValue = false

        let tNotif = NSAttributeDescription()
        tNotif.name = "notificationId"
        tNotif.attributeType = .stringAttributeType
        tNotif.isOptional = true

        timerEntity.properties = [tId, tTitle, tEnd, tRunning, tNotif]

        model.entities = [timeEntry, timerEntity]
        return model
    }
}
