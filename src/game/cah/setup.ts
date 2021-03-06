import { BaseCommandInteraction } from "discord.js";
import { MessageController } from "../../util/message";
import { GameInstance, shuffle } from "../game";
import { CAH, CAHState, getPlayerList, packs, randoId } from "./cah";

// Packs inside .gitignore
function conditionalRequire(name: string): any {
    try {
        return require(name);
    } catch (_) {}
    return undefined;
}

const eppgroep = conditionalRequire("./packs/eppgroep")?.eppgroep;

export function setup(game: GameInstance, state: CAHState, i: BaseCommandInteraction): Promise<void> {
    return new Promise<void>(resolve => {
        const internalPacks = [...packs];
        if (eppgroep && process.env.EPPGROEP && i.guildId === process.env.EPPGROEP) {
            internalPacks.push({ name: "EPPGroep", cards: eppgroep });
            internalPacks.push({ name: "EPPGroep", cards: eppgroep });
        }
    
        const packsPicked = internalPacks.map((_p, i) => i === 0);
    
        const msg = new MessageController(() => ({
            embeds: [{
                color: CAH.color,
                title: CAH.name,
                fields: [{
                    name: "Players",
                    value: getPlayerList(game.players, state.flags[0])
                }]
            }]
        }));

        game.join = (i, player) => {
            state.players[player.id] = {
                hand: [],
                playing: [],
                points: 0,
                hidden: false,
            };
            msg.updateAll(i);
        };
        game.leave = (i, player) => {
            delete state.players[player.id];
            msg.updateAll(i);
        };
        game.minPlayers = () => 2;
        game.maxPlayers = () => 20;
        
        game.addFlagsInput(msg, "Packs", internalPacks.map(p => p.name), packsPicked);
        game.addFlagsInput(msg, "Rules", ["Rando Cardrissian", "Quiplash Mode"], state.flags);
        game.addNumberInput(msg, "Points", 1, 8, Number.MAX_SAFE_INTEGER, value => state.maxPoints = value);
        game.addNumberInput(msg, "Cards", 5, 10, 20, value => state.handCards = value);
        game.setSetupMessage(msg, (i, m) => {
            // START
            state.whiteDeck = [];
            state.blackDeck = [];
    
            for (let i = 0; i < internalPacks.length; i++) {
                if (packsPicked[i]) {
                    for (const card of internalPacks[i].cards.white) state.whiteDeck.push(card);
                    for (const card of internalPacks[i].cards.black) state.blackDeck.push(card);
                }
            }
    
            if (state.blackDeck.length === 0) {
                i.reply({ ephemeral: true, content: "There are no black cards in the selected packs." });
                return;
            }
    
            if (state.whiteDeck.length < (state.flags[1] ? 0 : game.players.length * state.handCards) + (state.flags[0] ? 1 : 0)) {
                i.reply({ ephemeral: true, content: "There aren't enough white cards in the selected packs to give everyone a full hand." });
                return;
            }

            shuffle(state.whiteDeck);
            shuffle(state.blackDeck);

            if (state.flags[0]) {
                state.players[randoId] = {
                    hand: [],
                    playing: [],
                    points: 0,
                    hidden: false,
                }
            }

            msg.disableButtons(i, "_join", "_leave");
            game.resetControls();
            game.startLobby(m).then(() => resolve());
        });
    
        msg.reply(i);
    });
}
