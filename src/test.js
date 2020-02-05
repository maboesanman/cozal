const asyncOps = [
    "printAsync",
];

function dispatch_op(name, control) {
    op = Deno.core.ops()[name];
    if(asyncOps.includes(name)) {
        const p = new Promise((resolve) => {
            Deno.core.setAsyncHandler(op, (a) => {
                resolve(a);
            });
        });
        Deno.core.dispatch(op, control);
        return p;
    } else {
        return Deno.core.dispatch(op, control);
    }
}

a = dispatch_op("printSync", new Uint8Array([42]));
Deno.core.print(a[0]);

b = dispatch_op("printAsync", new Uint8Array([42]));
b.then((x) => Deno.core.print(x[0]));