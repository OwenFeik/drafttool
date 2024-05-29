enum Css {
    Card = "card",
    Center = "center",
    Hide = "hide",
    Label = "label",
    Selected = "selected",
}

type Card = {
    name: string,
    image: string,
    set: string,
    rarity: "Mythic" | "Rare" | "Uncommon" | "Common" | "Special" | "Bonus",
    text: string
};

type Status = "Ok" | "Warning" | "Error";

type PlayerDetails = {
    seat: string,
    name: string,
    ready: boolean,
    status: Status,
};

type PlayerList = PlayerDetails[];

type ServerMessage =
    { type: "Started" }
    | { type: "Ended" }
    | { type: "FatalError", value: string }
    | { type: "Pack", "value": Card[] }
    | { type: "PickSuccessful", "value": Card }
    | { type: "Finished", value: Card[] }
    | {
        type: "Connected",
        value: {
            draft: string,
            seat: string,
        }
    }
    | {
        type: "Reconnected",
        value: {
            draft: string,
            seat: string,
            in_progress: boolean,
            pool: Card[],
            pack?: Card[],
        }
    } | { type: "Refresh" }
    | {
        type: "PlayerUpdate",
        value: PlayerDetails,
    } | {
        type: "PlayerList",
        value: PlayerList
    };

type ClientMessage =
    { type: "HeartBeat" }
    | { type: "ReadyState", value: boolean }
    | { type: "Disconnected" }
    | { type: "SetName", value: string }
    | { type: "Pick", value: number };

enum Phase {
    Connecting,
    Lobby,
    Draft,
    Finished,
    Terminated,
}

type UiState =
    { phase: Phase.Connecting }
    | {
        phase: Phase.Lobby,
        updatePlayerList: (players: PlayerList) => void,
        updatePlayerDetails: (details: PlayerDetails) => void,
    }
    | {
        phase: Phase.Draft,
        receivePack: (pack: Card[]) => void,
        pickSuccessful: (picked: Card) => void,
        updatePlayerList: (players: PlayerList) => void,
        updatePlayerDetails: (details: PlayerDetails) => void,
        updatePool: (pool: Card[]) => void,
    }
    | {
        phase: Phase.Finished,
        updatePlayerList: (players: PlayerList) => void,
        updatePlayerDetails: (details: PlayerDetails) => void,
        updatePool: (pool: Card[]) => void,
    }
    | {
        phase: Phase.Terminated,
        displayErrorMessage: (message: string) => void,
    };

type State = {
    draft: string | null,
    seat: string | null,
    players: string[]
    playerDetails: Map<string, PlayerDetails>,
    ui: UiState,
    socket: WebSocket | null,
    reconnectAttempts: number,
};

let state: State = {
    draft: null,
    seat: null,
    players: [],
    playerDetails: new Map(),
    ui: { phase: Phase.Connecting },
    socket: null,
    reconnectAttempts: 0,
};

/**
 * Send a message throug the websocket to the server, if the websocket is open.
 * @param message Message to send on the socket to the server.
 * @returns true if the socket was open and the message sent, else false.
 */
function sendMessage(message: ClientMessage): boolean {
    if (state.socket == null) {
        console.warn("Not sending message as socket not available.", message);
        return false;
    }

    state.socket.send(JSON.stringify(message));
    return true;
}

function phaseRootElement(phase: Phase): HTMLElement {
    switch (phase) {
        case Phase.Connecting:
            return document.getElementById("connecting")!;
        case Phase.Lobby:
            return document.getElementById("lobby")!;
        case Phase.Draft:
            return document.getElementById("draft")!;
        case Phase.Finished:
            return document.getElementById("finished")!;
        case Phase.Terminated:
            return document.getElementById("terminated")!;
    }
}

function setVisible(el: HTMLElement, visible: boolean) {
    if (visible) {
        el.style.display = "unset";
    } else {
        el.style.display = "none";
    }
}

function el(tag: string, parent?: HTMLElement): HTMLElement {
    let element = document.createElement(tag);
    if (parent) {
        parent.appendChild(element);
    }
    return element;
}

function input(type: string, parent?: HTMLElement): HTMLInputElement {
    let element = attr(el("input", parent), "type", type);
    return element as HTMLInputElement;
}

function checkbox(parent?: HTMLElement): HTMLInputElement {
    return input("checkbox", parent);
}

function heading(parent: HTMLElement, content: string): HTMLElement {
    return text(
        classes(el("div", parent), "container-heading", "container-segment"),
        content
    );
}

function classes<E extends HTMLElement>(element: E, ...classes: string[]): E {
    element.classList.add(...classes);
    return element;
}

function text(element: HTMLElement, text: string): HTMLElement {
    element.innerText = text;
    return element;
}

function attr(element: HTMLElement, key: string, value?: string): HTMLElement {
    if (value !== undefined) {
        element.setAttribute(key, value);
    } else {
        element.toggleAttribute(key, true);
    }
    return element;
}

function forEachEl(selector: string, callback: (e: HTMLElement) => void) {
    document.querySelectorAll(selector)
        .forEach(el => callback(el as HTMLElement));
}

function toggleInput(
    parent: HTMLElement,
    initialValue: string,
    validate: (s: string) => string | true,
    onValidInput: (s: string) => void
): HTMLInputElement {
    let root = el("span", parent);
    let inp = classes(input("text", root), Css.Hide);
    inp.value = initialValue;
    let label = classes(text(el("span", root), initialValue), "label");
    let edit = classes(text(el("a", root), "[edit]"), "link-button");
    let save = classes(text(el("a", root), "[save]"), "link-button", Css.Hide);
    let cancel = text(el("a", root), "[cancel]");
    classes(cancel, "link-button", Css.Hide);

    edit.onclick = () => {
        inp.classList.remove(Css.Hide);
        edit.classList.add(Css.Hide);
        label.classList.add(Css.Hide);
        save.classList.remove(Css.Hide);
        cancel.classList.remove(Css.Hide);

        inp.focus();
        inp.select();
    };

    save.onclick = () => {
        let result = validate(inp.value);
        if (typeof result === "string") {
            inp.setCustomValidity(result);
        } else {
            inp.setCustomValidity("");
        }

        if (inp.validity.valid) {
            label.innerText = inp.value;
            inp.classList.add(Css.Hide);
            edit.classList.remove(Css.Hide);
            label.classList.remove(Css.Hide);
            save.classList.add(Css.Hide);
            cancel.classList.add(Css.Hide);
            onValidInput(inp.value);
        }
    };

    cancel.onclick = () => {
        inp.classList.add(Css.Hide);
        edit.classList.remove(Css.Hide);
        label.classList.remove(Css.Hide);
        save.classList.add(Css.Hide);
        cancel.classList.add(Css.Hide);
    };

    return inp;
}

function statusIndicator(parent: HTMLElement, status: Status): HTMLElement {
    let indicator = el("span", parent);
    classes(indicator, "player-status");
    updateStatusIndicator(indicator, status);
    return indicator;
}

function updateStatusIndicator(element: HTMLElement, status: Status) {
    element.classList.remove("ok", "warn", "err");
    element.classList.add(
        status == "Warning" ? "warn"
            : status == "Error" ? "err"
                : "ok"
    );
}

function cacheDisplayName(name: string) {
    if (state.seat == null) {
        return;
    }

    if (name != state.seat.substring(0, 8)) {
        localStorage.setItem("displayName", name);
    }
}

type UiPlayerListEntry = {
    seat: string,
    nameLabel: HTMLElement,
    nameInput?: HTMLInputElement,
    status: HTMLElement,
    ready?: HTMLInputElement,
};

type UiPlayerList = {
    entries: UiPlayerListEntry[],
    renderEntry: (details: PlayerDetails) => UiPlayerListEntry,
};

function statePlayerList(): PlayerList {
    let playerList: PlayerDetails[] = [];
    state.players.forEach(player => {
        let details = state.playerDetails.get(player);
        if (details != null) {
            playerList.push(details);
        }
    });
    return playerList;
}

function updatePlayerListEntry(details: PlayerDetails, ui: UiPlayerList) {
    if (details.seat == state.seat && state.ui.phase == Phase.Lobby) {
        // The default display name is the first 8 characters of the
        // seat ID. If this is our display name and we've previously
        // set a different display name, apply the one we've used in
        // the past.
        if (details.name == state.seat.substring(0, 8)) {
            let storedName = localStorage.getItem("displayName");
            if (storedName != null) {
                details.name = storedName;
                sendMessage({ type: "SetName", value: details.name });
            }
        }
    }

    let entry = ui.entries.find(player => player.seat == details.seat);
    if (entry === undefined) {
        entry = ui.renderEntry(details);
        ui.entries.push(entry);
        if (!state.players.includes(entry.seat)) {
            state.players.push(entry.seat);
        }
    }
    state.playerDetails.set(entry.seat, details);

    text(entry.nameLabel, details.name);
    updateStatusIndicator(entry.status, details.status);

    if (entry.nameInput) {
        entry.nameInput.value = details.name;
    }

    if (entry.ready) {
        entry.ready.checked = details.ready;
    }
}

function setUpLobby(root: HTMLElement): UiState {
    let float = classes(el("div", root), "floating-centered", "simple-border");
    let table = classes(el("table", float), "padded");
    let headrow = el("tr", el("thead", table));
    text(el("th", headrow), "Status");
    text(el("th", headrow), "User");
    text(el("th", headrow), "Ready");

    let playerList = el("tbody", table);
    let lobbyState: UiPlayerList = {
        entries: [],
        renderEntry: details => {
            let seat = details.seat;
            let row = attr(el("tr", playerList), "data-player", seat);

            let status = statusIndicator(
                classes(el("td", row), Css.Center),
                details.status
            );

            let nameCell = el("td", row);

            let readyCell = classes(el("td", row), Css.Center);
            let ready = checkbox(readyCell);
            ready.checked = details.ready;

            let nameInput: undefined | HTMLInputElement = undefined;

            if (seat == state.seat) {
                let name = details.name;
                nameInput = toggleInput(
                    nameCell,
                    name,
                    s => {
                        if (s.length < 1 || s.length > 32) {
                            return "Name must be 1-32 characters.";
                        } else {
                            return true;
                        }
                    },
                    value => {
                        cacheDisplayName(value);
                        sendMessage({ type: "SetName", value });
                    }
                );
                attr(nameInput, "minlength", "1");
                attr(nameInput, "maxlength", "32");
                nameInput.style.maxWidth = "100px";

                ready.oninput = () => sendMessage(
                    { type: "ReadyState", value: ready.checked }
                );
            } else {
                text(classes(el("span", nameCell), Css.Label), details.name);
                attr(ready, "disabled");
            }

            let nameLabel =
                nameCell.querySelector(`.${Css.Label}`) as HTMLElement;
            return { seat, nameLabel, nameInput, status, ready };
        }
    };

    const updatePlayerList = (players: PlayerList) => {
        playerList.innerHTML = "";
        state.players = [];
        state.playerDetails.clear();
        lobbyState.entries = [];
        players.forEach(details => updatePlayerListEntry(details, lobbyState));
    };

    // TODO request update if player missing from list.
    const updatePlayerDetails =
        (details: PlayerDetails) => updatePlayerListEntry(details, lobbyState);

    return {
        phase: Phase.Lobby,
        updatePlayerList,
        updatePlayerDetails,
    };
}

function renderCard(root: HTMLElement, card: Card): HTMLElement {
    let img = el("img", root);
    attr(img, "src", card.image);
    classes(img, Css.Card);
    return img;
}

function renderCardList(root: HTMLElement, cards: Card[]) {
    let index = 0;
    cards.forEach(card => attr(
        renderCard(root, card),
        "data-index",
        String(index++)
    ));
}

function populatePack(root: HTMLElement, cards: Card[]) {
    root.innerHTML = "";
    if (cards.length == 0) {
        heading(root, "Waiting for pack");
        return;
    }

    heading(root, "Current pack");
    renderCardList(root, cards);
    forEachEl(`.${Css.Card}`, img => img.onclick = e => {
        if (img.classList.contains(Css.Selected)) {
            if (img.dataset.index === undefined) {
                console.error("Card with no index. Can't pick.", img);
                return;
            }
            sendMessage({
                type: "Pick",
                value: parseInt(img.dataset.index)
            });
        } else {
            forEachEl(
                `.${Css.Card}.${Css.Selected}`,
                card => card.classList.remove(Css.Selected)
            );
            img.classList.add(Css.Selected);
        }
        e.stopPropagation();
    });
}

function renderDraftPlayers(root: HTMLElement):
    [(players: PlayerList) => void, (details: PlayerDetails) => void] {
    let list = el("span", root);

    let listState: UiPlayerList = {
        entries: [],
        renderEntry: details => {
            let entry = classes(el("span", list), "padhalf");
            let status = statusIndicator(entry, details.status);
            let nameLabel = text(el("span", entry), details.name);
            el("span", list).innerHTML = "&larr;";
            return { seat: details.seat, nameLabel, status };
        }
    };

    const updatePlayerList = (players: PlayerList) => {
        list.innerHTML = "";
        state.players = [];
        state.playerDetails.clear();
        listState.entries = [];
        players.forEach(details => updatePlayerListEntry(details, listState));
    };

    const updatePlayerDetails =
        (details: PlayerDetails) => updatePlayerListEntry(details, listState);

    return [updatePlayerList, updatePlayerDetails];
}

function renderCardWidthSelector(root: HTMLElement): (() => void) {
    let widthBox = el("span", root);
    text(classes(el("span", widthBox), "header-el"), "Card size");
    let width = el("input", widthBox) as HTMLInputElement;
    attr(width, "type", "range")
    attr(width, "min", "40");
    attr(width, "max", "400");
    attr(width, "value", "200");

    const updateCardWidths = () => {
        let w = width.value + "px";
        forEachEl(".card", card => card.style.width = w);
    };
    width.oninput = () => updateCardWidths();
    return updateCardWidths;
}

function setUpDraft(root: HTMLElement): UiState {
    let float = el("div", root);
    let header = classes(el("div", float), "container", "simple-border");
    heading(header, "Draft in progress");
    let pack = classes(el("div", float), "container", "simple-border");
    let pool = classes(el("div", float), "container", "simple-border");
    heading(pool, "Picked cards");

    let headerControls = el("div", header);
    classes(headerControls, "container-segment", "container-controls");
    const [updatePlayerList, updatePlayerDetails] =
        renderDraftPlayers(headerControls);
    updatePlayerList(statePlayerList());

    const updateCardWidths = renderCardWidthSelector(headerControls);

    pack.onclick = () => {
        forEachEl(
            `.${Css.Card}.${Css.Selected}`,
            card => card.classList.remove(Css.Selected)
        );
    };

    const receivePack = (cards: Card[]) => {
        populatePack(pack, cards);
        updateCardWidths();
    };

    const pickSuccessful = (card: Card) => {
        pack.innerHTML = "";
        heading(pack, "Waiting for pack");
        renderCard(pool, card);
        updateCardWidths();
    };

    const updatePool = (cards: Card[]) => {
        pool.innerHTML = "";
        heading(pool, "Picked cards");
        renderCardList(pool, cards);
        updateCardWidths();
    };

    return {
        phase: Phase.Draft,
        receivePack,
        pickSuccessful,
        updatePlayerList,
        updatePlayerDetails,
        updatePool,
    };
}

function setUpFinished(root: HTMLElement): UiState {
    let float = el("div", root);
    let header = classes(el("div", float), "container", "simple-border");
    heading(header, "Draft complete");
    let pool = classes(el("div", float), "container", "simple-border");

    const updateCardWidths = renderCardWidthSelector(header);

    const updatePool = (cards: Card[]) => {
        pool.innerHTML = "";
        heading(pool, "Picked cards");
        renderCardList(pool, cards);
        updateCardWidths();
    };

    return {
        phase: Phase.Finished,
        updatePlayerList: null!, // TODO
        updatePlayerDetails: null!, // TODO
        updatePool,
    };
}

function setUpTerminated(root: HTMLElement): UiState {
    let float = classes(el("div", root), "floating-centered", "simple-border");
    heading(float, "Draft terminated");
    let content = classes(el("div", float), "padded");
    text(el("span", content), "Error: ").style.fontWeight = "bold";
    let msg = el("span", content);

    const displayErrorMessage = (message: string) => {
        msg.innerText = message;
    };

    return {
        phase: Phase.Terminated,
        displayErrorMessage
    };
}

function moveToPhase(phase: Phase) {
    if (phase == state.ui.phase) {
        // Already in this phase; no need to change anything.
        return;
    }

    setVisible(phaseRootElement(state.ui.phase), false);

    let root = phaseRootElement(phase);
    root.innerHTML = ""; // Reset the phase UI if we've rendered it.
    setVisible(root, true);
    switch (phase) {
        case Phase.Lobby:
            state.ui = setUpLobby(root);
            break;
        case Phase.Draft:
            state.ui = setUpDraft(root);
            break;
        case Phase.Finished:
            state.ui = setUpFinished(root);
            break;
        case Phase.Terminated:
            state.ui = setUpTerminated(root);
            break;
    }
}

function displayErrorMessage(message: string) {
    console.error(message);
    if (state.ui.phase == Phase.Terminated) {
        state.ui.displayErrorMessage(message);
    } else {
        console.warn("Call to displayErrorMessage when phase not Terminated.");
    }
}

function terminate(message: string) {
    moveToPhase(Phase.Terminated);
    displayErrorMessage(message);
}

function receivedPack(pack: Card[]) {
    if (state.ui.phase == Phase.Draft) {
        state.ui.receivePack(pack);
    } else {
        console.warn("Can't display pack in phase", state.ui.phase);
    }
}

function pickSuccessful(picked: Card) {
    if (state.ui.phase == Phase.Draft) {
        state.ui.pickSuccessful(picked);
    } else {
        console.warn("Can't pass pack in phase", state.ui.phase);
    }
}

function updatePool(pool: Card[]) {
    if (state.ui.phase == Phase.Draft || state.ui.phase == Phase.Finished) {
        state.ui.updatePool(pool);
    } else {
        console.warn("Can't update pool in phase", state.ui.phase);
    }
}

function updatePlayerDetails(details: PlayerDetails) {
    switch (state.ui.phase) {
        case Phase.Lobby:
        case Phase.Draft:
        case Phase.Finished:
            state.ui.updatePlayerDetails(details);
            break;
        default:
            break;
    }
}

function updatePlayerList(playerList: PlayerList) {
    switch (state.ui.phase) {
        case Phase.Lobby:
        case Phase.Draft:
        case Phase.Finished:
            state.ui.updatePlayerList(playerList);
            break;
        default:
            console.warn("Can't update player list in phase", state.ui.phase);
            break;
    }
}

function updateDraftSeat(draft: string, seat: string) {
    state.draft = draft;
    state.seat = seat;
    seatToLocalStorage(draft, seat);
}

function handleMessage(message: ServerMessage) {
    switch (message.type) {
        case "Started":
            terminate("Failed to join draft. Draft has already started.");
            break;
        case "Ended":
            terminate("Failed to join draft. Draft already complete.");
            break;
        case "FatalError":
            terminate("Server error: " + message.value);
            break;
        case "Pack":
            moveToPhase(Phase.Draft);
            receivedPack(message.value);
            break;
        case "PickSuccessful":
            pickSuccessful(message.value);
            break;
        case "Finished":
            moveToPhase(Phase.Finished);
            updatePool(message.value);
            break;
        case "Connected":
            moveToPhase(Phase.Lobby);
            updateDraftSeat(message.value.draft, message.value.seat);
            break;
        case "Reconnected":
            let draft_in_progress = message.value.in_progress;
            moveToPhase(draft_in_progress ? Phase.Draft : Phase.Finished);
            updateDraftSeat(message.value.draft, message.value.seat);
            updatePool(message.value.pool);
            receivedPack(message.value.pack ? message.value.pack : []);
            break;
        case "Refresh":
            location.href = location.href;
            break;
        case "PlayerUpdate":
            updatePlayerDetails(message.value);
            break;
        case "PlayerList":
            updatePlayerList(message.value);
            break;
    }
}

function determineDraftId(): string | null {
    const url = new URL(location.href);
    const path = url.pathname;
    const parts = path.split("/");

    if (parts.length == 0) {
        return null;
    }

    const uuid = parts[parts.length - 1];

    if (!/^[0-9a-f\-]{36}$/.test(uuid)) {
        return null;
    }

    return uuid;
}

function seatFromLocalStorage(draftId: string): string | null {
    return localStorage.getItem(draftId);
}

function seatToLocalStorage(draftId: string, seatId: string) {
    localStorage.setItem(draftId, seatId);
}

function openWebsocket(draftId: string) {
    const MAX_RECONNECT_ATTEMPTS = 10;

    let protocol = location.protocol == "https" ? "wss" : "ws";
    let url = `${protocol}://${location.host}/ws/${draftId}`;
    let seatId = seatFromLocalStorage(draftId);
    if (seatId != null) {
        url = url + "/" + seatId;
    }

    let decoder = new TextDecoder("utf-8");

    const ws = new WebSocket(url);
    ws.binaryType = "arraybuffer";
    ws.onopen = e => {
        console.log("Websocket opened.");
        state.reconnectAttempts = 0;
        state.socket = ws;
    };
    ws.onmessage = e => handleMessage(JSON.parse(decoder.decode(e.data)));
    ws.onclose = e => {
        console.log("Websocket closed.");
        state.socket = null;
    };
    ws.onerror = e => {
        console.error("Websocket error:", e);
        if (state.reconnectAttempts < MAX_RECONNECT_ATTEMPTS) {
            state.reconnectAttempts++;
            openWebsocket(draftId);
        } else {
            console.log("Maximum number of reconnect attempts exceeded.");
            terminate("Connection error.");
        }
    };
}

function main() {
    const draftId = determineDraftId();
    if (draftId == null) {
        terminate("Draft ID not found in URL.");
        return;
    }

    openWebsocket(draftId);
}

window.onload = main
