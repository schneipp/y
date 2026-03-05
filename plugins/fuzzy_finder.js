// Fuzzy Finder Plugin - JavaScript implementation for Y Editor
// This plugin provides file finding and grep functionality

class FuzzyFinderPlugin extends Plugin {
    constructor() {
        super("fuzzy_finder");
        this.finderType = null; // "files" or "grep"
        this.query = "";
        this.results = [];
        this.selected = 0;
    }

    handleKey(keyEvent, context) {
        if (!this.active) {
            return { consumed: false, action: null };
        }

        const code = keyEvent.code;
        const modifiers = keyEvent.modifiers;

        // Escape to close
        if (code === "Esc") {
            this.deactivate();
            return {
                consumed: true,
                action: { type: "SetMode", mode: "Normal" }
            };
        }

        // Ctrl+n - next item
        if (code === "Char" && keyEvent.char === "n" && modifiers.includes("CONTROL")) {
            if (this.selected < this.results.length - 1) {
                this.selected++;
            }
            return { consumed: true, action: null };
        }

        // Ctrl+p - previous item
        if (code === "Char" && keyEvent.char === "p" && modifiers.includes("CONTROL")) {
            if (this.selected > 0) {
                this.selected--;
            }
            return { consumed: true, action: null };
        }

        // Down arrow - next item
        if (code === "Down") {
            if (this.selected < this.results.length - 1) {
                this.selected++;
            }
            return { consumed: true, action: null };
        }

        // Up arrow - previous item
        if (code === "Up") {
            if (this.selected > 0) {
                this.selected--;
            }
            return { consumed: true, action: null };
        }

        // Backspace - delete character from query
        if (code === "Backspace") {
            if (this.query.length > 0) {
                this.query = this.query.slice(0, -1);
                this.updateResults();
            }
            return { consumed: true, action: null };
        }

        // Enter - open selected file
        if (code === "Enter") {
            if (this.selected < this.results.length) {
                const selected = this.results[this.selected];
                return this.openResult(selected);
            }
            return { consumed: true, action: null };
        }

        // Regular character input
        if (code === "Char" && keyEvent.char) {
            this.query += keyEvent.char;
            this.updateResults();
            return { consumed: true, action: null };
        }

        return { consumed: false, action: null };
    }

    activate(finderType) {
        this.active = true;
        this.finderType = finderType;
        this.query = "";
        this.selected = 0;
        this.results = [];

        if (finderType === "files") {
            this.runRgFiles();
        } else if (finderType === "grep") {
            this.results = [];
        }
    }

    runRgFiles() {
        try {
            const output = YEditor.execCommand("rg", [
                "--files",
                "--hidden",
                "--glob", "!.git",
                "--glob", "!target",
                "--glob", "!node_modules",
                "--glob", "!.cache",
                "--glob", "!dist",
                "--glob", "!build",
            ]);
            this.results = output.split("\n").filter(line => line.length > 0).slice(0, 500);
        } catch (e) {
            YEditor.log("Failed to run rg --files: " + e);
            this.results = [];
        }
    }

    runRgGrep(query) {
        if (!query || query.length === 0) {
            this.results = [];
            return;
        }

        try {
            const output = YEditor.execCommand("rg", [
                "--line-number",
                "--column",
                "--no-heading",
                "--color=never",
                "--hidden",
                "--glob", "!.git",
                "--glob", "!target",
                "--glob", "!node_modules",
                "--glob", "!.cache",
                query
            ]);
            this.results = output.split("\n")
                .filter(line => line.length > 0)
                .slice(0, 100);
        } catch (e) {
            // rg returns error code if no matches found
            this.results = [];
        }
    }

    updateResults() {
        if (this.finderType === "files") {
            // Re-run and filter
            this.runRgFiles();
            const queryLower = this.query.toLowerCase();
            if (queryLower.length > 0) {
                this.results = this.results.filter(f =>
                    f.toLowerCase().includes(queryLower)
                );
            }
            this.results = this.results.slice(0, 100);
            this.selected = 0;
        } else if (this.finderType === "grep") {
            this.runRgGrep(this.query);
            this.selected = 0;
        }
    }

    openResult(result) {
        if (this.finderType === "files") {
            this.deactivate();
            return {
                consumed: true,
                action: { type: "OpenFile", path: result }
            };
        } else if (this.finderType === "grep") {
            // Parse: filename:line:col:text
            const parts = result.split(":");
            if (parts.length >= 3) {
                const filename = parts[0];
                const lineNum = parseInt(parts[1]) || 1;
                this.deactivate();
                return {
                    consumed: true,
                    action: {
                        type: "OpenFile",
                        path: filename,
                        line: lineNum - 1  // 0-indexed
                    }
                };
            }
        }
        return { consumed: true, action: null };
    }

    // This will be called from Rust to get render data
    getRenderData() {
        const title = this.finderType === "files" ? " Find Files " : " Find in Files ";

        return {
            active: this.active,
            title: title,
            query: this.query,
            results: this.results,
            selected: this.selected
        };
    }
}

// Create global instance
const fuzzyFinder = new FuzzyFinderPlugin();
