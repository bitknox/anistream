// Values as they cross mpv's client API, in mpv_node, and as JSON for the engine's reports.

import Foundation
import Mpv

/// A value as an `mpv_node` carries it: what properties read and take, what commands return,
/// and what an observed property changes to.
public enum MpvValue: Sendable, Hashable {
    case none
    case flag(Bool)
    case int(Int64)
    case double(Double)
    case string(String)
    case array([MpvValue])
    case map([String: MpvValue])
    case bytes(Data)

    /// Copied out of a node mpv owns, so it outlives the call or event it came in.
    init(_ node: mpv_node) {
        switch node.format {
        case MPV_FORMAT_STRING, MPV_FORMAT_OSD_STRING:
            self = node.u.string.map { .string(String(cString: $0)) } ?? .none
        case MPV_FORMAT_FLAG: self = .flag(node.u.flag != 0)
        case MPV_FORMAT_INT64: self = .int(node.u.int64)
        case MPV_FORMAT_DOUBLE: self = .double(node.u.double_)
        case MPV_FORMAT_NODE_ARRAY:
            guard let list = node.u.list?.pointee else {
                self = .array([])
                return
            }
            self = .array((0..<Int(list.num)).map { MpvValue(list.values[$0]) })
        case MPV_FORMAT_NODE_MAP:
            guard let list = node.u.list?.pointee else {
                self = .map([:])
                return
            }
            var map: [String: MpvValue] = [:]
            for i in 0..<Int(list.num) {
                guard let key = list.keys[i] else { continue }
                map[String(cString: key)] = MpvValue(list.values[i])
            }
            self = .map(map)
        case MPV_FORMAT_BYTE_ARRAY:
            guard let array = node.u.ba?.pointee, let data = array.data else {
                self = .bytes(Data())
                return
            }
            self = .bytes(Data(bytes: data, count: array.size))
        default:
            self = .none
        }
    }

    // MARK: Reading

    public var string: String? { if case .string(let s) = self { s } else { nil } }
    public var bool: Bool? { if case .flag(let b) = self { b } else { nil } }
    public var int: Int64? {
        switch self {
        case .int(let i): i
        case .double(let d) where d.isFinite: Int64(d)
        default: nil
        }
    }
    public var double: Double? {
        switch self {
        case .double(let d): d
        case .int(let i): Double(i)
        default: nil
        }
    }
    public var array: [MpvValue] { if case .array(let a) = self { a } else { [] } }
    public var map: [String: MpvValue] { if case .map(let m) = self { m } else { [:] } }
    public subscript(key: String) -> MpvValue { map[key] ?? .none }

    // MARK: JSON

    /// The value as JSON, the way mpv's own JSON IPC writes it; bytes are a byte count, since
    /// nothing reported carries them.
    public var json: String {
        guard let data = try? JSONSerialization.data(
            withJSONObject: jsonObject, options: [.fragmentsAllowed, .sortedKeys, .withoutEscapingSlashes])
        else { return "null" }
        return String(decoding: data, as: UTF8.self)
    }

    var jsonObject: Any {
        switch self {
        case .none: NSNull()
        case .flag(let b): b
        case .int(let i): i
        case .double(let d): d.isFinite ? d : NSNull()
        case .string(let s): s
        case .array(let a): a.map(\.jsonObject)
        case .map(let m): m.mapValues(\.jsonObject)
        case .bytes(let d): d.count
        }
    }

    /// A value from JSON, as the engine sends a `Set{property, value}`.
    public init(json: String) throws {
        let object = try JSONSerialization.jsonObject(with: Data(json.utf8), options: [.fragmentsAllowed])
        self.init(jsonObject: object)
    }

    init(jsonObject object: Any) {
        switch object {
        case is NSNull: self = .none
        case let number as NSNumber where CFGetTypeID(number) == CFBooleanGetTypeID():
            self = .flag(number.boolValue)
        case let number as NSNumber:
            // JSONSerialization keeps integers as integers; anything else is a double.
            if CFNumberIsFloatType(number) {
                self = .double(number.doubleValue)
            } else {
                self = .int(number.int64Value)
            }
        case let string as String: self = .string(string)
        case let array as [Any]: self = .array(array.map(MpvValue.init(jsonObject:)))
        case let map as [String: Any]: self = .map(map.mapValues(MpvValue.init(jsonObject:)))
        default: self = .none
        }
    }
}

extension MpvValue: ExpressibleByStringLiteral, ExpressibleByIntegerLiteral, ExpressibleByFloatLiteral,
    ExpressibleByBooleanLiteral, ExpressibleByArrayLiteral, ExpressibleByDictionaryLiteral
{
    public init(stringLiteral value: String) { self = .string(value) }
    public init(integerLiteral value: Int64) { self = .int(value) }
    public init(floatLiteral value: Double) { self = .double(value) }
    public init(booleanLiteral value: Bool) { self = .flag(value) }
    public init(arrayLiteral elements: MpvValue...) { self = .array(elements) }
    public init(dictionaryLiteral elements: (String, MpvValue)...) {
        self = .map(Dictionary(elements, uniquingKeysWith: { $1 }))
    }
}

/// The C memory of `mpv_node`s built from `MpvValue`s for one call, freed with it.
final class NodeArena {
    private var allocations: [UnsafeMutableRawPointer] = []

    deinit { allocations.forEach { free($0) } }

    func node(_ value: MpvValue) -> mpv_node {
        var node = mpv_node()
        switch value {
        case .none:
            node.format = MPV_FORMAT_NONE
        case .flag(let b):
            node.format = MPV_FORMAT_FLAG
            node.u.flag = b ? 1 : 0
        case .int(let i):
            node.format = MPV_FORMAT_INT64
            node.u.int64 = i
        case .double(let d):
            node.format = MPV_FORMAT_DOUBLE
            node.u.double_ = d
        case .string(let s):
            node.format = MPV_FORMAT_STRING
            node.u.string = cString(s)
        case .array(let values):
            node.format = MPV_FORMAT_NODE_ARRAY
            node.u.list = list(values.map(self.node), keys: nil)
        case .map(let map):
            node.format = MPV_FORMAT_NODE_MAP
            let keys = map.keys.sorted()
            node.u.list = list(keys.map { self.node(map[$0]!) }, keys: keys)
        case .bytes(let data):
            node.format = MPV_FORMAT_BYTE_ARRAY
            let array = allocate(mpv_byte_array.self, count: 1)
            let bytes = malloc(max(data.count, 1))!
            allocations.append(bytes)
            data.withUnsafeBytes { if let base = $0.baseAddress { memcpy(bytes, base, data.count) } }
            array.pointee = mpv_byte_array(data: bytes, size: data.count)
            node.u.ba = array
        }
        return node
    }

    func cString(_ string: String) -> UnsafeMutablePointer<CChar> {
        let pointer = strdup(string)!
        allocations.append(pointer)
        return pointer
    }

    private func list(_ values: [mpv_node], keys: [String]?) -> UnsafeMutablePointer<mpv_node_list> {
        let list = allocate(mpv_node_list.self, count: 1)
        let nodes = allocate(mpv_node.self, count: max(values.count, 1))
        for (i, value) in values.enumerated() { nodes[i] = value }
        var keyPointers: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>?
        if let keys {
            let pointers = allocate(UnsafeMutablePointer<CChar>?.self, count: max(keys.count, 1))
            for (i, key) in keys.enumerated() { pointers[i] = cString(key) }
            keyPointers = pointers
        }
        list.pointee = mpv_node_list(num: Int32(values.count), values: nodes, keys: keyPointers)
        return list
    }

    private func allocate<T>(_ type: T.Type, count: Int) -> UnsafeMutablePointer<T> {
        let raw = calloc(count, MemoryLayout<T>.stride)!
        allocations.append(raw)
        return raw.bindMemory(to: T.self, capacity: count)
    }
}
