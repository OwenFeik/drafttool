const FORM = [
    {
        name: "card_database",
        description: "Card database (Cockatrice XML)",
        type: "file",
        accept: ".xml",
        validate: input => (
            input.files.length == 1 || "Please select a card database file."
        )
    },
    {
        name: "list",
        description: "List of cards from the database to include in packs.",
        type: "file",
        accept: ".txt",
        validate: input => (
            input.files.length == 1 || "Please select a set list file."
        )
    },
    {
        name: "packs",
        description: "Number of packs in the draft.",
        type: "number",
        value: 3,
        validate: input => {
            let val = parseInt(input.value);
            return (Number.isInteger(val) && val > 0)
                || "Number of packs must be a positive integer.";
        }
    },
    {
        name: "cards_per_pack",
        description: "Number of cards in each pack.",
        type: "number",
        value: 15,
        validate: input => {
            let val = parseInt(input.value);
            return (Number.isInteger(val) && val > 0)
                || "Number of cards per pack must be a positive integer.";
        }
    },
    {
        name: "unique_cards",
        description: "Cards are unique (cube mode).",
        type: "checkbox",
        checked: true,
    },
    {
        name: "use_rarities",
        description: "Specify the number of each rarity in a pack.",
        type: "checkbox",
        checked: true,
        oninput: input => {
            set_field_visible("mythic_incidence", input.checked);
            set_field_visible("rares", input.checked);
            set_field_visible("uncommons", input.checked);
            set_field_visible("commons", input.checked);
        }
    },
    {
        name: "mythic_incidence",
        description: "Rate at which mythics replace rares in packs.",
        type: "number",
        value: 0.125,
        step: "any",
        validate: input => (
            (input.value >= 0.0 && input.value <= 1.0)
                || !get_value("use_rarities")
                || "Must be a probability in [0.0, 1.0]."
        )
    },
    {
        name: "rares",
        description: "Number of rares in each pack.",
        type: "number",
        value: 1,
        oninput: () => {
            get_input("uncommons").validate();
            get_input("commons").validate();
        },
        validate: validate_rarity,
    },
    {
        name: "uncommons",
        description: "Number of uncommons in each pack.",
        type: "number",
        value: 3,
        oninput: () => {
            get_input("rares").validate();
            get_input("commons").validate();
        },
        validate: validate_rarity,
    },
    {
        name: "commons",
        description: "Number of commons in each pack.",
        type: "number",
        value: 11,
        oninput: () => {
            get_input("rares").validate();
            get_input("uncommons").validate();
        },
        validate: validate_rarity,
    },
];

function validate_rarity(input) {
    if (!get_value("use_rarities")) {
        return true; // Value doesn't matter if rarities disabled.
    }
    
    let n = parseInt(input.value);
    if (!Number.isInteger(n) || n < 0) {
        return "Must be a number from 0 to number of cards per pack.";
    }

    let total = get_value("cards_per_pack");
    let rares = get_value("rares");
    let uncommons = get_value("uncommons");
    let commons = get_value("commons");
    if (rares + uncommons + commons != total) {
        return "Rares + uncommons + commons must equal cards per pack.";
    }

    return true;
}

function get_input(name) {
    return document.querySelector(`input[name="${name}"]`);
}

function get_value(name) {
    let input = get_input(name);
    if (input.type == "checkbox") {
        return input.checked;
    } else if (input.type == "number") {
        return parseFloat(input.value);
    } else {
        return input.value;
    }
}

function set_field_visible(name, visible) {
    let input = get_input(name);
    let row = input.parentElement;
    if (visible) {
        row.style.display = "";
    } else {
        row.style.display = "none";
    }
    
    if (input.validate) {
        input.validate();
    }
}

function build_form() {
    const form = document.getElementById("config");
    let inputs = [];
    FORM.forEach(field => {
        let row = document.createElement("div");
        row.classList.add("row");

        let label = document.createElement("span");
        label.innerText = field.description;
        row.appendChild(label);
        
        let input = document.createElement("input");
        input.name = field.name;
        input.type = field.type;
        if (field.checked) {
            input.checked = true;
        }
        if (field.value) {
            input.value = field.value;
        }
        if (field.accept) {
            input.accept = field.accept;
        }
        if (field.step) {
            input.step = field.step;
        }
        if (field.oninput) {
            input.addEventListener("input", () => field.oninput(input));
        }
        if (field.validate) {
            input.validate = () => {
                let validity = field.validate(input);
                if (typeof validity == "string") {
                    input.setCustomValidity(validity);
                    return false;
                } else {
                    input.setCustomValidity("");
                    return true;
                }
            };
            input.addEventListener("input", () => input.validate(input));
        }
        if (field.type == "checkbox") {
            input.addEventListener(
                "input",
                () => input.value = input.checked ? "checked" : "unchecked"
            );
            input.value = input.checked ? "checked" : "unchecked";
        }

        row.appendChild(input);
        inputs.push(input);

        form.appendChild(row);
    });

    let row = document.createElement("div");
    row.classList.add("row");
    let button = document.createElement("button");
    button.innerText = "Submit";
    button.onclick = () => {
        if (inputs.every(input => !input.validate || input.validate())) {
            form.submit();
        }
    };
    row.appendChild(button);
    form.appendChild(row);
}

window.onload = () => {
    build_form();
};
