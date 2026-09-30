import * as vscode from 'vscode';
import { RpcManager } from '../rpc/manager';
import { EditorStateManager } from '../state/EditorStateManager';
import { RequestPriority } from '../rpc/queue';

export interface PropertySchema {
    name: string;
    property_type: any; // Can be string "String", or object { Enum: ["A", "B"] }
    default_value: any;
    read_only: boolean;
}

export interface IPropertyEditor {
    render(schema: PropertySchema, value: any): string;
    // Client-side JS implementation script snippet for handling input and sending messages back
    getScript(): string; 
}

export class PropertyEditorRegistry {
    private editors = new Map<string, IPropertyEditor>();

    constructor() {
        this.register("String", new StringEditor());
        this.register("Number", new NumberEditor());
        this.register("Boolean", new BooleanEditor());
        this.register("Vector3", new Vector3Editor());
        this.register("Color3", new Color3Editor());
        this.register("Enum", new EnumEditor());
        this.register("Reference", new StringEditor());
    }

    public register(type: string, editor: IPropertyEditor) {
        this.editors.set(type, editor);
    }

    public getEditor(schema: PropertySchema): IPropertyEditor | undefined {
        const typeStr = typeof schema.property_type === 'string' ? schema.property_type : Object.keys(schema.property_type)[0];
        return this.editors.get(typeStr);
    }
}

// ------------------------------------------
// Concrete Editors (Vanilla JS + HTML)
// ------------------------------------------

export class StringEditor implements IPropertyEditor {
    render(schema: PropertySchema, value: any): string {
        const val = value !== undefined ? value : schema.default_value;
        const disabled = schema.read_only ? 'disabled' : '';
        return `
            <div class="property-row">
                <label>${schema.name}</label>
                <input type="text" id="prop-${schema.name}" value="${val}" ${disabled} onchange="updateProp('${schema.name}', this.value, 'String')"/>
            </div>
        `;
    }
    getScript(): string { return ""; } // General script handles updateProp
}

export class NumberEditor implements IPropertyEditor {
    render(schema: PropertySchema, value: any): string {
        const val = value !== undefined ? value : schema.default_value;
        const disabled = schema.read_only ? 'disabled' : '';
        return `
            <div class="property-row">
                <label>${schema.name}</label>
                <input type="number" step="any" id="prop-${schema.name}" value="${val}" ${disabled} onchange="updateProp('${schema.name}', parseFloat(this.value), 'Number')"/>
            </div>
        `;
    }
    getScript(): string { return ""; }
}

export class BooleanEditor implements IPropertyEditor {
    render(schema: PropertySchema, value: any): string {
        const val = value !== undefined ? value : schema.default_value;
        const disabled = schema.read_only ? 'disabled' : '';
        const checked = val ? 'checked' : '';
        return `
            <div class="property-row">
                <label>${schema.name}</label>
                <input type="checkbox" id="prop-${schema.name}" ${checked} ${disabled} onchange="updateProp('${schema.name}', this.checked, 'Boolean')"/>
            </div>
        `;
    }
    getScript(): string { return ""; }
}

export class Vector3Editor implements IPropertyEditor {
    render(schema: PropertySchema, value: any): string {
        const val = value || schema.default_value || {x:0, y:0, z:0};
        const disabled = schema.read_only ? 'disabled' : '';
        return `
            <div class="property-row vector3-row">
                <label>${schema.name}</label>
                <div class="vector-inputs">
                    X: <input type="number" step="any" id="prop-${schema.name}-x" value="${val.x}" ${disabled} onchange="updateVec3('${schema.name}')"/>
                    Y: <input type="number" step="any" id="prop-${schema.name}-y" value="${val.y}" ${disabled} onchange="updateVec3('${schema.name}')"/>
                    Z: <input type="number" step="any" id="prop-${schema.name}-z" value="${val.z}" ${disabled} onchange="updateVec3('${schema.name}')"/>
                </div>
            </div>
        `;
    }
    getScript(): string { 
        return `
            function updateVec3(name) {
                const x = parseFloat(document.getElementById('prop-' + name + '-x').value) || 0;
                const y = parseFloat(document.getElementById('prop-' + name + '-y').value) || 0;
                const z = parseFloat(document.getElementById('prop-' + name + '-z').value) || 0;
                updateProp(name, {x, y, z}, 'Vector3');
            }
        `; 
    }
}

export class Color3Editor implements IPropertyEditor {
    render(schema: PropertySchema, value: any): string {
        const val = value || schema.default_value || {r: 0, g: 0, b: 0};
        const disabled = schema.read_only ? 'disabled' : '';
        // Convert RGB to HEX for HTML color input
        const hex = "#" + (1 << 24 | val.r << 16 | val.g << 8 | val.b).toString(16).slice(1);
        return `
            <div class="property-row">
                <label>${schema.name}</label>
                <div style="display:flex; flex:1; gap:4px;">
                    <input type="color" id="prop-${schema.name}" value="${hex}" ${disabled} onchange="updateColor3('${schema.name}')" style="width: 40px; padding: 0;"/>
                    <div style="font-size:11px; align-self:center;">R:${val.r} G:${val.g} B:${val.b}</div>
                </div>
            </div>
        `;
    }
    getScript(): string { 
        return `
            function updateColor3(name) {
                const hex = document.getElementById('prop-' + name).value;
                const r = parseInt(hex.substr(1,2), 16);
                const g = parseInt(hex.substr(3,2), 16);
                const b = parseInt(hex.substr(5,2), 16);
                updateProp(name, {r, g, b}, 'Color3');
            }
        `;
    }
}

export class EnumEditor implements IPropertyEditor {
    render(schema: PropertySchema, value: any): string {
        const val = value !== undefined ? value : schema.default_value;
        const disabled = schema.read_only ? 'disabled' : '';
        const options = schema.property_type.Enum as string[];
        let optionsHtml = options.map(opt => `<option value="${opt}" ${opt === val ? 'selected' : ''}>${opt}</option>`).join('');
        return `
            <div class="property-row">
                <label>${schema.name}</label>
                <select id="prop-${schema.name}" ${disabled} onchange="updateProp('${schema.name}', this.value, 'Enum')" style="flex:1; background: var(--vscode-input-background); color: var(--vscode-input-foreground); border: 1px solid var(--vscode-input-border); padding: 4px;">
                    ${optionsHtml}
                </select>
            </div>
        `;
    }
    getScript(): string { return ""; }
}
