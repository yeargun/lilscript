export function createDocument() {
  const doc = {
    createElement(tag) {
      return createEl(tag);
    },
    createElementNS(_ns, tag) {
      return createEl(tag);
    },
    createTextNode(data) {
      const node = createEl(null);
      node.nodeType = 3;
      node.data = String(data);
      node.nodeName = "#text";
      return node;
    },
  };

  function createEl(tag) {
    const el = {
      nodeType: tag == null ? 3 : 1,
      nodeName: tag == null ? "#text" : String(tag).toUpperCase(),
      localName: tag == null ? undefined : String(tag),
      parentNode: null,
      nextSibling: null,
      previousSibling: null,
      firstChild: null,
      lastChild: null,
      childNodes: [],
      attributes: [],
      style: {},
      ownerDocument: doc,
      namespaceURI: "http://www.w3.org/1999/xhtml",
      __attrs: Object.create(null),
      l: null,
      setAttribute(name, value) {
        this.__attrs[name] = String(value);
        const existing = this.attributes.find((attr) => attr.name === name);
        if (existing) existing.value = String(value);
        else this.attributes.push({ name, value: String(value) });
        if (name === "class") this.className = String(value);
        if (name === "id") this.id = String(value);
      },
      removeAttribute(name) {
        delete this.__attrs[name];
        this.attributes = this.attributes.filter((attr) => attr.name !== name);
      },
      getAttribute(name) {
        return name in this.__attrs ? this.__attrs[name] : null;
      },
      addEventListener() {},
      removeEventListener() {},
      contains(node) {
        let current = node;
        while (current) {
          if (current === this) return true;
          current = current.parentNode;
        }
        return false;
      },
      appendChild(child) {
        return this.insertBefore(child, null);
      },
      removeChild(child) {
        const index = this.childNodes.indexOf(child);
        if (index < 0) return child;
        this.childNodes.splice(index, 1);
        if (child.previousSibling) {
          child.previousSibling.nextSibling = child.nextSibling;
        } else {
          this.firstChild = child.nextSibling;
        }
        if (child.nextSibling) {
          child.nextSibling.previousSibling = child.previousSibling;
        } else {
          this.lastChild = child.previousSibling;
        }
        child.parentNode = null;
        child.nextSibling = null;
        child.previousSibling = null;
        return child;
      },
      insertBefore(child, before) {
        if (child.parentNode) child.parentNode.removeChild(child);
        child.parentNode = this;
        if (before == null) {
          if (this.lastChild) {
            this.lastChild.nextSibling = child;
            child.previousSibling = this.lastChild;
          } else {
            this.firstChild = child;
          }
          this.lastChild = child;
          this.childNodes.push(child);
        } else {
          const index = this.childNodes.indexOf(before);
          this.childNodes.splice(index, 0, child);
          child.nextSibling = before;
          child.previousSibling = before.previousSibling;
          if (before.previousSibling) {
            before.previousSibling.nextSibling = child;
          } else {
            this.firstChild = child;
          }
          before.previousSibling = child;
        }
        return child;
      },
      remove() {
        if (this.parentNode) this.parentNode.removeChild(this);
      },
    };
    Object.defineProperty(el, "textContent", {
      get() {
        if (this.nodeType === 3) return this.data || "";
        return this.childNodes.map((child) => child.textContent).join("");
      },
      set(value) {
        if (this.nodeType === 3) {
          this.data = String(value);
          return;
        }
        while (this.firstChild) this.removeChild(this.firstChild);
        if (value) this.appendChild(doc.createTextNode(String(value)));
      },
    });
    Object.defineProperty(el, "value", {
      get() {
        return this.__value || "";
      },
      set(value) {
        this.__value = value;
      },
    });
    Object.defineProperty(el, "checked", {
      get() {
        return !!this.__checked;
      },
      set(value) {
        this.__checked = !!value;
      },
    });
    return el;
  }

  return doc;
}
