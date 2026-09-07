#!/usr/bin/env -S uv run --script
"""Prepare and summarize live conversation trials; provider calls use the Rust host.

uv run scripts/evaluate_conversation_continuity.py prepare --output /tmp/conversation-trials.json
cargo run -p cathedral-backends --example conversation_eval -- \
    --requests /tmp/conversation-trials.json --output /tmp/conversation-results
uv run scripts/evaluate_conversation_continuity.py summarize /tmp/conversation-results

Counts below measure speaking/quiet only. Inspect raw content before acceptance.
"""
import argparse
import json
import re
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
EVIDENCE = ROOT / "features/conversation_continuity_evidence"
if not EVIDENCE.exists():
    EVIDENCE = ROOT / "features/implemented/conversation_continuity_evidence"

POLICY = '''Speech in your history is what you could hear, not necessarily speech addressed
to you. A [Conversation: ...] note records apparent attention when that line
was spoken. It is evidence, not an order: the words may name another person,
invite the group, or clearly answer an interjection. Decide who is addressed
before deciding how to answer.

An ordinary follow-up belongs to the person exchanging replies with the
speaker. If someone just described their work and is asked "How will you do
that?", do not answer about YOUR work merely because you also have a trade.
When you are overhearing, normally use `wait {}`. You need no turn merely
because you heard a question, know an answer, or happen to stand closer.

You may enter someone else's exchange for a concrete reason of your own:
a danger you witness, a relevant correction you can support, a debt or other
personal stake, an objection, or a particular intrusive motive in your
character. Speak to that reason and make its connection to the exchange
clear; do not take over the role of the person being questioned. Do not
invent an emergency, grievance, or special knowledge to justify speaking.

If the words address you, or invite everyone and you have a relevant answer,
answer naturally. If the player takes up your interjection, you may continue
that exchange. A tentative attention note must not silence an explicitly
invited listener. A name mentioned inside a story is not itself an address.
If the intended listener is genuinely unclear, judge from the actual words
and what you witnessed; a brief clarification can be natural.

'''

FOOTER = '''Take one or more actions.
First decide whether the latest speech addresses you, invites the group, or
belongs to an exchange you are overhearing. If it is another person's turn,
use wait unless you have a concrete reason to interject; speak to that reason.
If answering a question addressed to you, answer the latest unanswered one
with say, using your own sheet. For "there", use the place you named to that
asker before their question; later bystander remarks do not change it.
'''

OVERHEAR = ' [Conversation: probable addressee: a stranger (id g9sib); you are overhearing; evidence: ongoing exchange.]'
SELF = ' [Conversation: probable addressee: you; evidence: ongoing exchange.]'
GROUP = ' [Conversation: open question to nearby listeners, including you.]'
GAZE = ' [Conversation: probable addressee: you; evidence: sustained attention.]'

COURTESY = '''A [Conversation: ...] note records apparent attention when a line was spoken.
The actual words take precedence: if they address you by name or invite
everyone, you may answer even when the note suggests another listener.
Otherwise, a follow-up normally continues the exchange already underway.
Someone asking another person about their work, plans, home or belongings
is not asking about yours. Having a similar trade or a nearby home does not
make that question yours. An interjection should speak to your particular
reason for entering the exchange, rather than adopting the addressee's role.

'''

COURTESY_FOOTER = '''Take one or more actions.
Make SURE that what you're doing matches what you see, who you are, what you can think about/understand etc.
First read the words: if they address you or invite the group, answer when
you have a relevant answer. Otherwise respect the exchange you are overhearing;
use wait unless you have a concrete reason to interject, and speak to that reason.
When answering a question addressed to you, find the asker's latest unanswered
question in since_your_last_turn. For "there", look back to YOUR last answer
to that asker before the question; use the place you named there. Later
bystander speech is not that question's context. Answer with say, using your sheet.
'''

ATTENTION = '''Sustained attention to you is a sign that a plain "you" or "your" question
is addressed to you, even if somebody else spoke last. For example, if you
warned the player and another person repeated the warning, the player can
turn to you and ask your name. Answer that question when their attention is
on you; an explicit name or group invitation in the words still takes precedence.

'''


def replace_section(prompt, name, value):
    return re.sub(r'^\*\*' + re.escape(name) + r'\*\*.*?(?=\n\n|\Z)',
                  lambda _: value, prompt, count=1, flags=re.M | re.S)


def fixtures():
    return {p.stem: json.loads(p.read_text())["prompt"]
            for p in (EVIDENCE / 'fixtures').glob('*.json')}


def scenarios():
    f = fixtures()
    question = 'How are you going to trade?'
    player = '- A stranger (id player) said: "{}"'
    base = f['rohese']
    cases = []

    def add(name, prompt, last, cue, expected):
        cases.append(dict(name=name, prompt=prompt, last=last, cue=cue, expected=expected))

    add('sibbe_bystander', base, player.format(question), OVERHEAR, 'quiet')
    sibbe = replace_section(f['sibbe'], 'since_your_last_turn',
                            '**since_your_last_turn**:\n' + player.format(question))
    add('sibbe_answer', sibbe, player.format(question), SELF, 'speak')
    for name, line, cue, expected in [
        ('named_switch', 'Rohese, what do you trade?', GAZE, 'speak'),
        ('named_overrides_hint', 'Rohese, what do you trade?', OVERHEAR, 'speak'),
        ('group_question', 'Does anyone here sell spices?', GROUP, 'speak'),
        ('group_overrides_hint', 'Does anyone here sell spices?', OVERHEAR, 'speak'),
        ('mentioned_name', "Sibbe, when you pass Rohese's stall, where will you go next?", OVERHEAR, 'quiet'),
        ('deliberate_gaze_switch', 'And you, what do you trade?', GAZE, 'speak'),
    ]:
        add(name, base.replace(question, line), player.format(line), cue, expected)

    danger = base.replace(player.format(question), player.format(question) +
                          '\n- You see a heavy roof tile break loose directly above the player. There is time to warn them before it falls.')
    add('witnessed_danger', danger, player.format(question), OVERHEAR, 'speak')

    debt = replace_section(base, 'stored_memories',
        '**stored_memories**:\n- The person with id g9sib borrowed my four sparks yesterday, promised to repay me this morning, and has not repaid me. I need them back to buy my stock today.')
    debt = debt.replace('Aye? Sibbe\'s here.', "I will use those four sparks I borrowed from you, Rohese, to buy my eels. You can wait another week.")
    add('personal_stake', debt, player.format(question), OVERHEAR, 'speak')

    invited_line = 'That sounds useful, Rohese. What spices do you trade?'
    invited = base.replace(player.format(question),
        '- You said to a stranger (id player): "I trade spices at the Wickmarket, if you need any."\n' + player.format(invited_line))
    add('accepted_interjection', invited, player.format(invited_line), SELF, 'speak')

    alone = replace_section(base, 'you_see', '**you_see** (people within 20 metres, nearest first):\n- id player: a stranger (you don\'t know their name), 2.3 m')
    alone = replace_section(alone, 'since_your_last_turn', '**since_your_last_turn**:\n' + player.format('What do you trade?'))
    add('new_exchange', alone, player.format('What do you trade?'),
        ' [Conversation: probable addressee: you; evidence: proximity only.]', 'speak')

    nosy = base.replace('You bargain cheerfully and remember exact sums.',
        'You bargain cheerfully and remember exact sums. You are an incorrigible busybody: today you specifically want to find out whether this eel trader is carrying spoiled fish, because your sister became ill after buying eels yesterday. You have no evidence that this trader was responsible.')
    add('intrusive_motive', nosy, player.format(question), OVERHEAR, 'either')
    for name, line in [('petronel', "What's the closest place"),
                       ('aldith', 'Okay what other places?'),
                       ('lark', 'Hey, why did you go to Cindeward?')]:
        cue = ' [Conversation: probable addressee: a stranger (id x00560); you are overhearing; evidence: ongoing exchange.]'
        add(name + '_archive', f[name], player.format(line), cue, 'quiet')
    for i, line in enumerate(['So what are you going to sell?', 'What do you trade?', 'And where are you going?']):
        add(f'followup_{i+1}', base.replace(question, line), player.format(line), OVERHEAR, 'quiet')
    if 'accepted_after_second_warning' in f:
        add('accepted_after_second_warning', f['accepted_after_second_warning'],
            'Thank you for warning me. What is your name?', '', 'speak')
    return cases


def apply_policy(prompt):
    start = prompt.index('Speech in your history is what you could hear')
    end = prompt.index('what_you_know is the whole', start)
    prompt = prompt[:start] + POLICY + prompt[end:]
    return prompt[:prompt.rindex('Take one or more actions.')] + FOOTER


def prepare(args):
    trials = []
    variants = args.variants.split(',')
    for case in scenarios():
        if args.cases and case['name'] not in args.cases.split(','):
            continue
        for variant in variants:
            prompt = case['prompt']
            if variant in ('context', 'roles', 'courtesy', 'shipped', 'attentive'):
                assert case['last'] in prompt
                prompt = prompt.replace(case['last'], case['last'] + case['cue'])
            if variant == 'roles':
                prompt = apply_policy(prompt)
            if variant == 'courtesy':
                start = prompt.index('Speech in your history is what you could hear')
                prompt = prompt[:start] + COURTESY + prompt[start:]
                prompt = prompt[:prompt.rindex('Take one or more actions.')] + COURTESY_FOOTER
            if variant in ('shipped', 'attentive'):
                # The archived scene remains fixed while the shipped social
                # rules/footer change. Full-engine trials separately verify rendering.
                template = (ROOT / 'assets/prompts/turn.j2').read_text()
                start = template.index('Speech in your history is what you could hear')
                end = template.index('what_you_know is the whole', start)
                old_start = prompt.index('Speech in your history is what you could hear')
                old_end = prompt.index('what_you_know is the whole', old_start)
                prompt = prompt[:old_start] + template[start:end] + prompt[old_end:]
                prompt = prompt[:prompt.rindex('Take one or more actions.')] + template[template.rindex('Take one or more actions.'):]
                if variant == 'attentive' and 'Sustained attention to you is a sign' not in prompt:
                    prompt = prompt.replace('Before speaking in response, decide', ATTENTION + 'Before speaking in response, decide')
            for repeat in range(1, args.repeats + 1):
                trials.append(dict(name=f"{variant}__{case['name']}__r{repeat}",
                                   prompt=prompt, max_output_tokens=350))
    args.output.write_text(json.dumps(trials, ensure_ascii=False, indent=2) + '\n')
    print(f'{len(trials)} trials written to {args.output}')


def summarize(args):
    expected = {case['name']: case['expected'] for case in scenarios()}
    counts = defaultdict(lambda: defaultdict(int))
    for path in sorted(args.directory.glob('*.json')):
        d = json.loads(path.read_text())
        if 'answer' not in d:
            continue
        variant, case, repeat = d['name'].split('__')
        answer = d['answer'] or ''
        spoke = bool(re.search(r'^say\s*\{', answer, re.M))
        valid = bool(re.search(r'^(say|wait|go_to|remember|forget|set_goal|gesture|make_sound)\s*\{', answer, re.M))
        outcome = 'error' if d['error'] else 'invalid' if not valid else 'speak' if spoke else 'quiet'
        counts[(variant, case)][outcome] += 1
        if args.replies:
            print(f"{d['name']} [{expected[case]} / {outcome}] {answer}")
    for (variant, case), count in sorted(counts.items()):
        print(f'{variant:9} {case:25} expected={expected[case]:5} {dict(count)}')


def write_scenes(args):
    def person(actor_id, name, story, x, z, control='llm'):
        return dict(id=actor_id, name=name, back_story=story, control=control,
                    location_description='Beside the Shambles well', voice_key=None,
                    position_m=dict(x=x, y=0.91, z=z), holds=[], memories=[], knows=[])
    seed = dict(characters=[
        person('g9sib', 'Sibbe Skell', 'You are an eel and herring trader. Today you are heading to Coswald\'s Yard to sell herring for one spark each and smoked eel for three sparks each. You bargain briskly but honestly.', 0, 4),
        person('p009a', 'Rohese Nett', 'You are a grocer and spicer. Today you are heading to the Wickmarket to sell spices and sealed oils. You bargain cheerfully and remember exact sums. You do not sell fish.', 9, 4),
        person('x00428', 'Bertran Pike', 'You are a quiet elderly resident resting beside the well. You have no business with these traders and no goods for sale.', -8, 5),
        person('player', 'Player', 'A human visitor.', 0, 0, 'player'),
    ])
    opening = dict(label='establish_sibbe', text='Sibbe, where are you going to trade?',
                   focus='g9sib', focus_seconds=0.7, turns=1)
    nearer = [['p009a', [0, 0.91, 1.4]]]
    scenes = {
        'continuity_and_switches': [opening,
            dict(label='nearer_bystander_followup', moves=nearer, text='How are you going to trade?', turns=3),
            dict(label='open_group', text='Does anyone here sell spices?', turns=3),
            dict(label='named_switch', text='Rohese, where do you trade?', turns=3),
            dict(label='switch_followup', text='And what will you sell there?', turns=3),
            dict(label='return_to_sibbe', text='Sibbe, how much is your smoked eel?', turns=3)],
        'interruption_and_acceptance': [opening,
            dict(label='unsolicited_greeting', moves=nearer,
                 npc_lines=[['p009a', 'Good day, stranger.']],
                 text='So what are you going to sell?', turns=3),
            dict(label='witnessed_danger',
                 percepts=[['p009a', 'You see a heavy roof tile break loose directly above the player. There is time to warn them before it falls.']],
                 text='And what does your herring cost?', turns=3),
            dict(label='accept_interjection', focus='p009a', focus_seconds=0.7,
                 text='Thank you for warning me. What is your name?', turns=3),
            dict(label='new_partner_followup', text='What do you do for a living?', turns=3)],
        'attention_and_departure': [opening,
            dict(label='brief_glance', focus='p009a', focus_seconds=0.2, moves=nearer,
                 text='What are you selling?', turns=3),
            dict(label='sustained_attention', focus='p009a', focus_seconds=0.7,
                 text='And you, what do you trade?', turns=3),
            dict(label='partner_departs', moves=[['p009a', [80, 0.91, 80]]],
                 text='Sibbe, are you still here?', turns=2),
            dict(label='silence_then_new_person', silence_seconds=35,
                 moves=[['g9sib', [80, 0.91, 85]], ['p009a', [0, 0.91, 1.4]]],
                 text='Hello. What is your name?', turns=2)],
    }
    args.output.mkdir(parents=True, exist_ok=True)
    for name, steps in scenes.items():
        (args.output / f'{name}.json').write_text(json.dumps(dict(seed=seed, steps=steps), ensure_ascii=False, indent=2) + '\n')
    print(f'{len(scenes)} Engine scene scripts written to {args.output}')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest='command', required=True)
    prep = sub.add_parser('prepare')
    prep.add_argument('--output', type=Path, required=True)
    prep.add_argument('--variants', default='baseline,context,roles')
    prep.add_argument('--repeats', type=int, default=3)
    prep.add_argument('--cases')
    prep.set_defaults(run=prepare)
    summary = sub.add_parser('summarize')
    summary.add_argument('directory', type=Path)
    summary.add_argument('--replies', action='store_true')
    summary.set_defaults(run=summarize)
    scenes = sub.add_parser('scenes')
    scenes.add_argument('--output', type=Path, required=True)
    scenes.set_defaults(run=write_scenes)
    args = parser.parse_args()
    args.run(args)


if __name__ == '__main__':
    main()
