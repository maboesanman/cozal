// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

// A script preamble that provides the ability to load a single outfile
// TypeScript "bundle" where a main module is loaded which recursively
// instantiates all the other modules in the bundle.  This code is used to load
// bundles when creating snapshots, but is also used when emitting bundles from
// Deno cli.

// @ts-nocheck

/**
 * @type {(name: string, deps: ReadonlyArray<string>, factory: (...deps: any[]) => void) => void=}
 */
// eslint-disable-next-line @typescript-eslint/no-unused-vars
let define;

/**
 * @type {(mod: string) => any=}
 */
// eslint-disable-next-line @typescript-eslint/no-unused-vars
let instantiate;

/**
 * @callback Factory
 * @argument {...any[]} args
 * @returns {object | void}
 */

/**
 * @typedef ModuleMetaData
 * @property {ReadonlyArray<string>} dependencies
 * @property {(Factory | object)=} factory
 * @property {object} exports
 */

(function() {
  /**
   * @type {Map<string, ModuleMetaData>}
   */
  const modules = new Map();

  /**
   * Bundles in theory can support "dynamic" imports, but for internal bundles
   * we can't go outside to fetch any modules that haven't been statically
   * defined.
   * @param {string[]} deps
   * @param {(...deps: any[]) => void} resolve
   * @param {(err: any) => void} reject
   */
  const require = (deps, resolve, reject) => {
    try {
      if (deps.length !== 1) {
        throw new TypeError("Expected only a single module specifier.");
      }
      if (!modules.has(deps[0])) {
        throw new RangeError(`Module "${deps[0]}" not defined.`);
      }
      resolve(getExports(deps[0]));
    } catch (e) {
      if (reject) {
        reject(e);
      } else {
        throw e;
      }
    }
  };

  define = (id, dependencies, factory) => {
    if (modules.has(id)) {
      throw new RangeError(`Module "${id}" has already been defined.`);
    }
    modules.set(id, {
      dependencies,
      factory,
      exports: {}
    });
  };

  /**
   * @param {string} id
   * @returns {any}
   */
  function getExports(id) {
    const module = modules.get(id);
    if (!module) {
      // because `$deno$/ts_global.d.ts` looks like a real script, it doesn't
      // get erased from output as an import, but it doesn't get defined, so
      // we don't have a cache for it, so because this is an internal bundle
      // we can just safely return an empty object literal.
      return {};
    }
    if (!module.factory) {
      return module.exports;
    } else if (module.factory) {
      const { factory, exports } = module;
      delete module.factory;
      if (typeof factory === "function") {
        const dependencies = module.dependencies.map(id => {
          if (id === "require") {
            return require;
          } else if (id === "exports") {
            return exports;
          }
          return getExports(id);
        });
        factory(...dependencies);
      } else {
        Object.assign(exports, factory);
      }
      return exports;
    }
  }

  instantiate = dep => {
    define = undefined;
    const result = getExports(dep);
    // clean up, or otherwise these end up in the runtime environment
    instantiate = undefined;
    return result;
  };
})();

var __importDefault = (this && this.__importDefault) || function (mod) {
    return (mod && mod.__esModule) ? mod : { "default": mod };
};
define("utilities/immutable-priority-queue", ["require", "exports"], function (require, exports) {
    "use strict";
    Object.defineProperty(exports, "__esModule", { value: true });
    function lessThan(keys1, keys2) {
        let i = 0;
        while (true) {
            let key1 = 0;
            if (i < keys1.length && keys1[i] != undefined)
                key1 = keys1[i];
            let key2 = 0;
            if (i < keys2.length && keys2[i] != undefined)
                key2 = keys2[i];
            if (key1 != key2)
                return key1 < key2;
            if (i >= keys1.length && i >= keys2.length)
                return false; // equal
            i = i + 1;
        }
    }
    function heapInsert(node, keys, value) {
        if (node === undefined)
            return { count: 1, keys: keys, value };
        // convert the count + 1 to binary, and the digits after the first 1 represent
        // the position the new element should be inserted
        const direction = (node.count + 1).toString(2)[1];
        if (direction === '0') {
            if (lessThan(keys, node.keys)) {
                return {
                    keys: keys,
                    value,
                    count: node.count + 1,
                    left: heapInsert(node.left, node.keys, node.value),
                    right: node.right,
                };
            }
            else {
                return {
                    keys: node.keys,
                    value: node.value,
                    count: node.count + 1,
                    left: heapInsert(node.left, keys, value),
                    right: node.right,
                };
            }
        }
        else {
            if (lessThan(keys, node.keys)) {
                return {
                    keys: keys,
                    value,
                    count: node.count + 1,
                    left: node.left,
                    right: heapInsert(node.right, node.keys, node.value),
                };
            }
            else {
                return {
                    keys: node.keys,
                    value: node.value,
                    count: node.count + 1,
                    left: node.left,
                    right: heapInsert(node.right, keys, value),
                };
            }
        }
    }
    function heapExtract(node) {
        if (node === undefined)
            return { node: undefined };
        const removeDirection = (node.count).toString(2)[1] == "0" ? "Left" : "Right";
        const bubbleDirection = heapGetBubbleDownDirection(node);
        if (removeDirection === bubbleDirection && removeDirection === "Left") {
            const { node: left, keys, value } = heapExtract(node.left);
            return {
                node: {
                    ...node,
                    count: node.count - 1,
                    left,
                },
                keys,
                value,
            };
        }
        if (removeDirection === bubbleDirection && removeDirection === "Right") {
            const { node: right, keys, value } = heapExtract(node.right);
            return {
                node: {
                    ...node,
                    count: node.count - 1,
                    right,
                },
                keys,
                value,
            };
        }
        const { node: lastNode, keys, value } = heapRemoveLast(node);
        const newNode = heapBubbleDown(lastNode, { keys, value });
        return {
            node: newNode,
            keys: node.keys,
            value: node.value,
        };
    }
    function heapRemoveLast(node) {
        if (node === undefined)
            throw new Error("something tragic has happened");
        if (node.count === 1) {
            return {
                keys: node.keys,
                value: node.value
            };
        }
        // convert the count + 1 to binary, and the digits after the first 1 represent
        // the position the new element should be inserted
        const direction = (node.count).toString(2)[1];
        if (direction === '0') { // Left
            const { node: left, keys, value } = heapRemoveLast(node.left);
            return {
                node: {
                    ...node,
                    count: node.count - 1,
                    left,
                },
                keys,
                value,
            };
        }
        else { // Right
            const { node: right, keys, value } = heapRemoveLast(node.right);
            return {
                node: {
                    ...node,
                    count: node.count - 1,
                    right,
                },
                keys,
                value,
            };
        }
    }
    function heapGetBubbleDownDirection(node, compareKeys = undefined) {
        if (node === undefined)
            return "Neither";
        let potentials = {};
        const finalCompareKeys = compareKeys !== undefined ? compareKeys : node.keys;
        if (node.left !== undefined && lessThan(node.left.keys, finalCompareKeys))
            potentials.left = node.left.keys;
        if (node.right !== undefined && lessThan(node.right.keys, finalCompareKeys))
            potentials.right = node.right.keys;
        if (potentials.left !== undefined && potentials.right !== undefined) {
            if (lessThan(potentials.left, potentials.right))
                potentials.right = undefined;
            else
                potentials.left = undefined;
        }
        if (potentials.left !== undefined && potentials.right === undefined) {
            return "Left";
        }
        if (potentials.left === undefined && potentials.right !== undefined) {
            return "Right";
        }
        return "Neither";
    }
    function heapBubbleDown(node, replace = undefined) {
        if (node === undefined)
            return undefined;
        const compareKey = replace === undefined ? node.keys : replace.keys;
        const direction = heapGetBubbleDownDirection(node, compareKey);
        switch (direction) {
            case "Left":
                return {
                    keys: node.left.keys,
                    value: node.left.value,
                    count: node.count,
                    left: heapBubbleDown(node.left, { keys: node.keys, value: node.value, ...replace }),
                    right: node.right,
                };
            case "Right":
                return {
                    keys: node.right.keys,
                    value: node.right.value,
                    count: node.count,
                    left: node.left,
                    right: heapBubbleDown(node.right, { keys: node.keys, value: node.value, ...replace }),
                };
            default:
                return { ...node, ...replace };
        }
    }
    class PriorityQueue {
        constructor(...keyNames) {
            this.Root = undefined;
            this.KeyNames = keyNames.map(keyName => {
                if (Array.isArray(keyName))
                    return keyName;
                return [keyName, "ascending"];
            });
        }
        static fromRoot(root, keyNames) {
            // manually assign to readonly property. this is an alternate private constructor.
            const result = new PriorityQueue(...keyNames);
            result.Root = root;
            return result;
        }
        insert(value) {
            const keys = this.KeyNames.map((keyName) => {
                if (keyName[1] === "ascending")
                    return (value[keyName[0]]);
                else
                    return (-value[keyName[0]]);
            });
            const heap = heapInsert(this.Root, keys, value);
            return PriorityQueue.fromRoot(heap, this.KeyNames);
        }
        peek() {
            if (this.Root === undefined)
                return undefined;
            return this.Root.value;
        }
        dequeue() {
            const { node: heap } = heapExtract(this.Root);
            return PriorityQueue.fromRoot(heap, this.KeyNames);
        }
        insertRange(items) {
            let queue = this;
            items.forEach(item => {
                queue = queue.insert(item);
            });
            return queue;
        }
        get count() {
            if (this.Root === undefined)
                return 0;
            return this.Root.count;
        }
    }
    exports.default = PriorityQueue;
});
define("utilities/immutable-stack", ["require", "exports"], function (require, exports) {
    "use strict";
    Object.defineProperty(exports, "__esModule", { value: true });
    class Stack {
        static fromHead(head) {
            // manually assign to readonly property. this is an alternate private constructor.
            const result = new Stack();
            result.head = head;
            return result;
        }
        push(value) {
            return Stack.fromHead({ value, next: this.head });
        }
        pop() {
            return Stack.fromHead(this.head?.next);
        }
        peek() {
            return this.head?.value;
        }
    }
    exports.default = Stack;
});
define("types/state", ["require", "exports"], function (require, exports) {
    "use strict";
    Object.defineProperty(exports, "__esModule", { value: true });
});
define("types/cozal-event", ["require", "exports"], function (require, exports) {
    "use strict";
    Object.defineProperty(exports, "__esModule", { value: true });
});
define("initialize-logic", ["require", "exports"], function (require, exports) {
    "use strict";
    Object.defineProperty(exports, "__esModule", { value: true });
    function initializeLogic(name, initFn) {
    }
    exports.default = initializeLogic;
});
define("initialize-representer", ["require", "exports"], function (require, exports) {
    "use strict";
    Object.defineProperty(exports, "__esModule", { value: true });
    // you can use this separately for different things, like video and audio for example
    // or possibly to break out BGAs into their own representation.
    function initializeRepresenter(name, initFn) {
    }
    exports.default = initializeRepresenter;
});
define("types/cozal-system-event", ["require", "exports"], function (require, exports) {
    "use strict";
    Object.defineProperty(exports, "__esModule", { value: true });
});
define("cozal", ["require", "exports", "utilities/immutable-priority-queue", "utilities/immutable-stack", "initialize-logic", "initialize-representer"], function (require, exports, immutable_priority_queue_ts_1, immutable_stack_ts_1, initialize_logic_ts_1, initialize_representer_ts_1) {
    "use strict";
    Object.defineProperty(exports, "__esModule", { value: true });
    immutable_priority_queue_ts_1 = __importDefault(immutable_priority_queue_ts_1);
    immutable_stack_ts_1 = __importDefault(immutable_stack_ts_1);
    initialize_logic_ts_1 = __importDefault(initialize_logic_ts_1);
    initialize_representer_ts_1 = __importDefault(initialize_representer_ts_1);
    const CozalInternal = {
        cozalHistory: new immutable_stack_ts_1.default().push({
            event: {
                source: 'system',
                time: 0,
                sort: 0,
                id: 0
            },
            futureEvents: new immutable_priority_queue_ts_1.default("time", "sort", "id"),
            logicStates: {},
            representationStates: {},
        }),
        handleEvent(event) {
            // rollback if necessary
            // call logic event handlers
            // call representation event handlers
            // push cozal frame
        },
    };
    const Cozal = {
        initializeLogic: initialize_logic_ts_1.default,
        initializeRepresenter: initialize_representer_ts_1.default,
    };
    exports.default = Cozal;
    // an example logic 
    Cozal.initializeLogic('secondCounter', (ctx) => {
        ctx.addEvent({
            time: 1000,
            sort: -1,
            message: 'hello',
        });
        ctx.setState({
            count: 0,
        });
        return (ctx, event) => {
            ctx.setState({
                count: ctx.state.count + 1,
            });
            ctx.addEvent({
                time: event.time + 1000,
                sort: -1,
                message: 'hello',
            });
        };
    });
    // an example representer
    Cozal.initializeRepresenter('secondMessenger', (ctx) => {
        return (ctx, event) => {
            Deno.core.print(event.message);
        };
    });
});

const __rootExports = instantiate("cozal");
export default __rootExports["default"];
