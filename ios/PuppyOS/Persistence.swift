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
        
        let entity = NSEntityDescription()
        entity.name = "TimeEntryEntity"
        entity.managedObjectClassName = NSStringFromClass(TimeEntryEntity.self)
        
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
        
        entity.properties = [id, title, start, end]
        model.entities = [entity]
        return model
    }
}
