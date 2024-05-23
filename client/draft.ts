type Card = {
    name: string,
    image: string,
    set: string,
    rarity: "Mythic" | "Rare" | "Uncommon" | "Common" | "Special" | "Bonus",
    text: string
};

type ServerMessage =
    { type: "Started" }
    | { type: "Ended" }
    | { type: "FatalError", value: string }
    | { type: "Pack", "value": Card[] }
    | { type: "Passed" }
    | { type: "Finished", value: Card[] }
    | {
        type: "Connected",
        value: {
            draft: string,
            seat: string,
            players: [string, string][],
        }
    }
    | {
        type: "Reconnected",
        value: {
            draft: string,
            seat: string,
            pool: Card[],
            pack?: Card[],
        }
    };

enum Phase {
    Lobby,
    Draft,
    Finished,
    Terminated,
}

type State = {
    phase: Phase,
    pack?: Card[],
    pool: Card[],
    reconnectAttempts: number,
};

let state: State = {
    phase: Phase.Lobby,
    pool: [],
    reconnectAttempts: 0,
};

function moveToPhase(phase: Phase) {
    console.log("Move to phase:", phase);
    // TODO set up page with appropriate UI for the phase
}

function displayErrorMessage(message: string) {
    console.log("Server error:", message);
    // TODO display error to user
}

function receivedPack(pack: Card[]) {
    console.log("Received pack:", pack);
    // TODO update current pack to allow user to pick
}

function passedPack() {
    console.log("Passed pack.");
    // TODO clear current pack from the interface
}

function updatePool(pool: Card[]) {
    console.log("Update pool:", pool);
    // TODO update the users pool of picked cards
}

function updatePlayerList(playerList: [string, string][]) {
    console.log("Update player list:", playerList);
    // TODO update the list of players in the UI
}

function handleMessage(message: ServerMessage) {
    switch (message.type) {
        case "Started":
            break; // TODO failed to join because draft already started.
        case "Ended":
            moveToPhase(Phase.Finished);
            break;
        case "FatalError":
            displayErrorMessage(message.value);
            moveToPhase(Phase.Terminated);
            break;
        case "Pack":
            receivedPack(message.value);
            break;
        case "Passed":
            passedPack();
            break;
        case "Finished":
            moveToPhase(Phase.Finished);
            updatePool(message.value);
            break;
        case "Connected":
            moveToPhase(Phase.Lobby);
            seatToLocalStorage(message.value.draft, message.value.seat);
            updatePlayerList(message.value.players);
            break;
        case "Reconnected":
            // TODO way to identify if draft is in progress or complete
            seatToLocalStorage(message.value.draft, message.value.seat);
            updatePool(message.value.pool);
            if (message.value.pack) {
                receivedPack(message.value.pack);
            }
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

    const ws = new WebSocket(url);
    ws.onopen = e => {
        console.log("Websocket opened.");
        state.reconnectAttempts = 0;
    };
    ws.onmessage = e => {
        e.data.text().then((json: string) => handleMessage(JSON.parse(json)));
    };
    ws.onerror = e => {
        console.error("Websocket error:", e);
        if (state.reconnectAttempts < MAX_RECONNECT_ATTEMPTS) {
            state.reconnectAttempts++;
            openWebsocket(draftId);
        } else {
            console.log("Maximum number of reconnect attempts exceeded.");
            displayErrorMessage("Connection error.");
        }
    };
}

function main() {
    const draftId = determineDraftId();
    if (draftId == null) {
        return;
    }

    openWebsocket(draftId);
}

window.onload = main
