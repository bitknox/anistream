/* tslint:disable */
/* eslint-disable */

/**
 * A store, open on the platform's storage.
 */
export class Store {
    private constructor();
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Answer one request.
     *
     * `request`: `{ method, path, headers: [[name, value]…], body: Uint8Array, client, base,
     * now }`, where `base` is the store's address — given, or this request's own origin — and
     * `now` is seconds since the epoch. Answers `{ status, headers: [[name, value]…], body }`.
     */
    handle(request: any): Promise<any>;
    /**
     * Open the store, creating its tables and key on first use.
     *
     * `files` may be `null`, and plugin files then live in the database too — what a platform
     * with a database and no blob store of its own needs. `options`: `{ name?, url?,
     * setupCode?, trustDigests? }`.
     */
    static open(db: any, files: any | null | undefined, options: any): Promise<Store>;
}

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_store_free: (a: number, b: number) => void;
    readonly store_handle: (a: number, b: any) => any;
    readonly store_open: (a: any, b: number, c: any) => any;
    readonly wasm_bindgen_3839f7469b67aab5___convert__closures_____invoke___wasm_bindgen_3839f7469b67aab5___JsValue__core_37ee0ce0c4cd57d5___result__Result_____wasm_bindgen_3839f7469b67aab5___JsError___true_: (a: number, b: number, c: any) => [number, number];
    readonly wasm_bindgen_3839f7469b67aab5___convert__closures_____invoke___js_sys_c45c4ad8eacb7336___Function_fn_wasm_bindgen_3839f7469b67aab5___JsValue_____wasm_bindgen_3839f7469b67aab5___sys__Undefined___js_sys_c45c4ad8eacb7336___Function_fn_wasm_bindgen_3839f7469b67aab5___JsValue_____wasm_bindgen_3839f7469b67aab5___sys__Undefined_______true_: (a: number, b: number, c: any, d: any) => void;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_exn_store: (a: number) => void;
    readonly __externref_table_alloc: () => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_destroy_closure: (a: number, b: number) => void;
    readonly __externref_table_dealloc: (a: number) => void;
    readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
