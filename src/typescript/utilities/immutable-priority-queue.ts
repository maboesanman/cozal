type FilterFlags<Base, Condition> = {
    [Key in keyof Base]: 
        Base[Key] extends Condition ? Key : never
};

// keys which map to numbers
type FilteredKeys<Base, Condition> = 
    FilterFlags<Base, Condition>[keyof Base];

interface HeapNodeValue<T> {
    readonly keys: number[];
    readonly value: T;
    readonly count: number;
    readonly left?: HeapNode<T>;
    readonly right?: HeapNode<T>;
}

type HeapNode<T> = HeapNodeValue<T> | undefined;
function lessThan(keys1: number[], keys2: number[]) {
    let i = 0;
    while(true) {
        let key1 = 0;
        if(i < keys1.length && keys1[i] != undefined)
            key1 = keys1[i];

        let key2 = 0;
        if(i < keys2.length && keys2[i] != undefined)
            key2 = keys2[i];

        if(key1 != key2)
            return key1 < key2;

        if(i >= keys1.length && i >= keys2.length)
            return false; // equal

        i = i + 1;
    }
}
function heapInsert<T>(node: HeapNode<T>, keys: number[], value: T) : HeapNode<T> {
    if(node === undefined)
        return { count: 1, keys: keys, value };
    
    // convert the count + 1 to binary, and the digits after the first 1 represent
    // the position the new element should be inserted
    const direction = (node.count + 1).toString(2)[1];
    if(direction === '0') {
        if(lessThan(keys, node.keys)) {
            return {
                keys: keys,
                value,
                count: node.count + 1,
                left: heapInsert(node.left, node.keys, node.value),
                right: node.right,
            }
        } else {
            return {
                keys: node.keys,
                value: node.value,
                count: node.count + 1,
                left: heapInsert(node.left, keys, value),
                right: node.right,
            }
        }
    } else {
        if(lessThan(keys, node.keys)) {
            return {
                keys: keys,
                value,
                count: node.count + 1,
                left: node.left,
                right: heapInsert(node.right, node.keys, node.value),
            }
        } else {
            return {
                keys: node.keys,
                value: node.value,
                count: node.count + 1,
                left: node.left,
                right: heapInsert(node.right, keys, value),
            }
        }
    }
}

function heapExtract<T>(node: HeapNode<T>): {
    node: HeapNode<T>;
    keys?: number[];
    value?: T | undefined;
} {
    if(node === undefined)
        return { node: undefined }

    const removeDirection = (node.count).toString(2)[1] == "0" ? "Left" : "Right";
    const bubbleDirection = heapGetBubbleDownDirection(node);

    if(removeDirection === bubbleDirection && removeDirection === "Left") {
        const {node: left, keys, value} = heapExtract(node.left);
        return {
            node: {
                ...node,
                count: node.count - 1,
                left,
            },
            keys,
            value,
        }
    }
    if(removeDirection === bubbleDirection && removeDirection === "Right") {
        const {node: right, keys, value} = heapExtract(node.right);
        return {
            node: {
                ...node,
                count: node.count - 1,
                right,
            },
            keys,
            value,
        }
    }

    const {node: lastNode, keys, value} = heapRemoveLast(node);
    const newNode = heapBubbleDown(lastNode, { keys, value });

    return {
        node: newNode,
        keys: node.keys,
        value: node.value,
    }
}

function heapRemoveLast<T>(node: HeapNode<T>): {
    node?: HeapNode<T>;
    keys: number[];
    value: T;
} {
    if(node === undefined)
        throw new Error("something tragic has happened");

    if(node.count === 1) {
        return {
            keys: node.keys,
            value: node.value
        }
    }
    // convert the count + 1 to binary, and the digits after the first 1 represent
    // the position the new element should be inserted
    const direction = (node.count).toString(2)[1];
    if(direction === '0') { // Left
        const {node: left, keys, value } = heapRemoveLast(node.left);
        return {
            node: {
                ...node,
                count: node.count - 1,
                left,
            },
            keys,
            value,
        }
    } else { // Right
        const {node: right, keys, value } = heapRemoveLast(node.right);
        return {
            node: {
                ...node,
                count: node.count - 1,
                right,
            },
            keys,
            value,
        }
    }
}

function heapGetBubbleDownDirection<T>(node: HeapNode<T>, compareKeys: number[] | undefined = undefined): "Left" | "Right" | "Neither" {
    if(node === undefined)
        return "Neither";

    let potentials: {
        left?: number[];
        right?: number[];
    } = {};

    const finalCompareKeys = compareKeys !== undefined ? compareKeys : node.keys;
    
    if(node.left !== undefined && lessThan(node.left.keys, finalCompareKeys))
        potentials.left = node.left.keys;
    
    if(node.right !== undefined && lessThan(node.right.keys, finalCompareKeys))
        potentials.right = node.right.keys;
    
    if(potentials.left !== undefined && potentials.right !== undefined) {
        if(lessThan(potentials.left, potentials.right))
            potentials.right = undefined;
        else
            potentials.left = undefined;
    }
    if(potentials.left !== undefined && potentials.right === undefined) {
        return "Left";
    }
    if(potentials.left === undefined && potentials.right !== undefined) {
        return "Right";
    }
    return "Neither";
}

function heapBubbleDown<T>(node: HeapNode<T>, replace: { keys: number[], value: T } | undefined = undefined): HeapNode<T> {
    if(node === undefined)
        return undefined;
    
    const compareKey = replace === undefined ? node.keys : replace.keys;
    const direction = heapGetBubbleDownDirection(node, compareKey); 

    switch (direction) {
        case "Left":
            return {
                keys: node.left!.keys,
                value: node.left!.value,
                count: node.count,
                left: heapBubbleDown(node.left, { keys: node.keys, value: node.value, ...replace} ),
                right: node.right,
            };
        case "Right":
            return {
                keys: node.right!.keys,
                value: node.right!.value,
                count: node.count,
                left: node.left,
                right: heapBubbleDown(node.right, { keys: node.keys, value: node.value, ...replace} ),
            };
        default:
            return { ...node, ...replace };
    }
}
type KeyName<T> = FilteredKeys<T, number | undefined>;
type KeyTuple<T> = [KeyName<T>, "ascending" | "descending"];

export default class PriorityQueue<T> {
    private readonly Root: HeapNode<T>;
    private readonly KeyNames: KeyTuple<T>[];

    public constructor(...keyNames: (KeyName<T> | KeyTuple<T>)[]) {
        this.Root = undefined;
        this.KeyNames = keyNames.map(keyName => {
            if(Array.isArray(keyName))
                return keyName;
            return [keyName, "ascending"] as KeyTuple<T>;
        });
    }

    private static fromRoot<T>(root: HeapNode<T>, keyNames: KeyTuple<T>[]): PriorityQueue<T> {
        // manually assign to readonly property. this is an alternate private constructor.
        const result = new PriorityQueue<T>(...keyNames) as any;
        result.Root = root;
        return result as PriorityQueue<T>;
    }

    public insert(value: T): PriorityQueue<T> {
        const keys = this.KeyNames.map((keyName) => {
            if(keyName[1] === "ascending")
                return (value[keyName[0]]) as unknown as number
            else
                return (-value[keyName[0]]) as unknown as number
        });
        const heap = heapInsert<T>(this.Root, keys, value);
        return PriorityQueue.fromRoot(heap, this.KeyNames);
    }

    public peek(): T | undefined {
        if(this.Root === undefined)
            return undefined;
        
        return this.Root.value;
    }

    public dequeue(): PriorityQueue<T> {
        const { node: heap } = heapExtract(this.Root);
        return PriorityQueue.fromRoot(heap, this.KeyNames);
    }

    public insertRange(items: T[]): PriorityQueue<T> {
        let queue: PriorityQueue<T> = this;
        items.forEach(item => {
            queue = queue.insert(item);
        });
        return queue;
    }

    get count(): number {
        if(this.Root === undefined)
            return 0;

        return this.Root.count;
    }
}