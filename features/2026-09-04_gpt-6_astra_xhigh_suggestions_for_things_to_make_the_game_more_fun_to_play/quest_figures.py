"""Nine schematic figures for An Alibi in Stone; all proposed, not game captures."""
from __future__ import annotations
import json
from pathlib import Path
import matplotlib.pyplot as plt
from matplotlib.patches import Circle, Rectangle, Polygon

INK = "#263D42"
GREEN = "#426B60"
GOLD = "#B3873F"
RED = "#AA604A"
PALE = "#E8EEE8"
SAND = "#F0E4CB"
PAPER = "#FBF8F1"
MUTED = "#626E70"
LINE = "#C7D1CB"


def make_figures(canvas, box, label, arrow, save):
    model = json.loads((Path(__file__).parent / "quest_model.json").read_text())
    fig, ax = canvas(3.8)
    x = lambda t: 1.12 + t / 70 * 8.4
    ax.axvspan(x(0), x(36), color=PALE, zorder=0)
    for t in (0, 17, 36, 60):
        ax.plot([x(t)]*2, [.57,3.05], color=LINE, lw=1)
        label(ax,x(t),.33,str(t),10,MUTED)
    for y, name, start, end, color in [
        (2.62,"Lise",0,36,GOLD),
        (1.95,"Corin",3,33.86,GREEN),
        (1.18,"Warin",0,36,INK)]:
        label(ax,.15,y,name,11,INK,"bold",ha="left")
        ax.plot([x(start),x(end)],[y,y],lw=9,color=color,solid_capstyle="butt")
    label(ax,x(18),2.91,"In yard · counter unseen",10)
    label(ax,x(18.4),2.19,"Room → rack → counter",10)
    label(ax,x(18),1.43,"At the lower beam",10)
    ax.scatter([x(0),x(36)],[2.62,2.62],color=GOLD,s=85,zorder=5,edgecolors=INK)
    ax.scatter([x(17)],[1.18],color=RED,s=75,zorder=5)
    label(ax,x(47),1.95,"Odo cries out\nnear t = 17",10,RED)
    arrow(ax,(x(39),1.91),(x(17),1.18),RED)
    label(ax,x(61),2.8,"Warin finds\nOdo later",10)
    arrow(ax,(x(61),2.45),(x(60),.8),GOLD)
    label(ax,5,3.52,"AUTHOR TIMELINE · ELAPSED SIMULATION SECONDS",11,GREEN,"bold")
    save(fig,"incident_timeline")

    fig,ax=canvas(4.4)
    box(ax,.25,2.65,2.25,1.02,"A · PAWN COUNTER","Two sightings",size=10)
    box(ax,7.35,2.65,2.35,1.02,"C · ODO'S ROOM","The attack",size=10)
    box(ax,3.8,3.12,2.43,.91,"UPPER LINK","Private · 22 m outward",size=10)
    arrow(ax,(2.55,3.18),(3.75,3.57)); arrow(ax,(6.28,3.57),(7.3,3.18))
    box(ax,3.85,1.88,2.33,.84,"H · LOFT RACK","The packet",color=SAND,edge=GOLD,size=10)
    arrow(ax,(7.3,2.84),(6.25,2.35),GOLD)
    arrow(ax,(3.8,2.28),(2.55,2.83),GOLD)
    label(ax,6.8,2.23,"6 m",10,GOLD)
    label(ax,3.0,2.15,"20 m",10,GOLD)
    ax.plot([.25,.08,.08,9.9,9.9,9.7],[2.92,2.92,.82,.82,2.92,2.92],color=RED,lw=2.2)
    label(ax,5,.51,"PUBLIC ENTRANCES · 576 m RETURN TARGET",11,RED,"bold")
    box(ax,7.45,1.25,2.12,.75,"W / G · BELOW","Beam and writer",color=PAPER,edge=LINE,size=9)
    arrow(ax,(8.5,2.57),(8.5,2.08),GOLD,style="--")
    label(ax,6.85,1.57,"Cry through\ngrilled light",9,MUTED)
    label(ax,1.4,2.12,"Y · Hoist yard\nCounter obscured",10)
    save(fig,"case_topology")

    fig=plt.figure(figsize=(10,3.5)); ax=fig.add_axes([.24,.21,.72,.60])
    minimum=model["sighting_gap_seconds"]["minimum"]
    maximum=model["sighting_gap_seconds"]["maximum"]
    values=[48,48/2.1+8]
    ax.axvspan(minimum,maximum,color=SAND,zorder=0)
    ax.barh([1,0],values,color=[RED,GREEN],height=.44)
    ax.set(xlim=(0,56),ylim=(-.55,1.65),yticks=[1,0],
           yticklabels=["Public · fastest bound","Private · normal NPC"],xticks=[0,10,20,30,40,50],xlabel="Elapsed simulation seconds")
    for y,v in zip([1,0],values): ax.text(v+1,y,f"{v:.1f} s",va="center",fontsize=11,color=INK)
    ax.text(36,1.48,"Observed gap\n32–40 s",ha="center",fontsize=10,color=GOLD)
    ax.spines[["top","right","left"]].set_visible(False); ax.spines["bottom"].set_color(LINE)
    ax.tick_params(axis="y",length=0); ax.grid(axis="x",color=LINE,alpha=.3)
    fig.text(.5,.93,"THE SAME GAP · TWO DIFFERENT NETWORKS",ha="center",fontsize=11,color=GREEN,weight="bold")
    save(fig,"route_timing")

    fig,ax=canvas(4.0)
    ax.add_patch(Rectangle((2.5,.65),5.15,2.75,facecolor=PALE,edgecolor=INK,lw=2))
    ax.plot([2.5,2.5],[1.05,1.65],color=PAPER,lw=5)
    ax.plot([7.65,7.65],[2.28,2.9],color=PAPER,lw=5)
    ax.add_patch(Rectangle((4.15,1.67),1.6,.65,facecolor=SAND,edgecolor=GOLD,lw=1.5))
    label(ax,4.95,2.0,"DESK",10,INK,"bold")
    ax.add_patch(Rectangle((5.4,1.17),.43,.4,angle=-20,facecolor=PAPER,edgecolor=INK))
    label(ax,6.4,1.05,"Fallen stool",10)
    ax.scatter([4.24],[1.55],color=RED,s=42)
    label(ax,3.41,1.15,"Logged catch\nfragment",10,RED)
    arrow(ax,(3.77,1.35),(4.19,1.53),RED)
    ax.plot([4.45,5.8],[3.4,3.4],color=GOLD,lw=5)
    for xx in [4.6,4.9,5.2,5.5]: ax.plot([xx,xx],[3.25,3.53],color=INK,lw=1.1)
    label(ax,5.12,3.75,"Grilled back light · sound to beam below",10)
    label(ax,1.18,1.38,"Public stair\nfrom lane",11)
    arrow(ax,(1.78,1.38),(2.85,1.38),RED)
    label(ax,8.72,2.65,"Service latch\nfrom loft",11)
    arrow(ax,(8.12,2.65),(7.24,2.65),GREEN)
    label(ax,5,.25,"Inspectable connections, not a hidden-pixel search",10,MUTED)
    save(fig,"scene_plan")

    fig,ax=canvas(3.6)
    for i,(title,body) in enumerate([
        ("BEFORE","Lise sees Corin\nat the counter"),
        ("THE UNSEEN INTERVAL","Lise tends the hoist\nA wall blocks her view"),
        ("AFTER","Lise sees Corin\nat the same counter")]):
        xx=.2+i*3.32
        box(ax,xx,.62,2.85,2.55,"",color=SAND if i==1 else PALE)
        label(ax,xx+1.425,2.89,title,10,GREEN,"bold")
        if i==1:
            ax.plot([xx+1.2]*2,[1.54,2.38],color=INK,lw=5)
            ax.add_patch(Circle((xx+.7,1.95),.13,color=GOLD))
            ax.add_patch(Circle((xx+2.1,1.95),.13,color=GREEN))
            ax.plot([xx+.85,xx+1.15],[1.95,1.95],color=GOLD,ls="--",lw=2)
            label(ax,xx+1.45,1.25,body,10)
        else:
            ax.add_patch(Rectangle((xx+.62,1.83),1.6,.17,facecolor=GOLD))
            ax.add_patch(Circle((xx+1.425,2.24),.15,color=GREEN))
            label(ax,xx+1.425,1.25,body,10)
    label(ax,5,.23,"TWO TRUE SIGHTINGS DO NOT ESTABLISH CONTINUOUS PRESENCE",10,GREEN,"bold")
    save(fig,"alibi_sightline")

    fig,ax=canvas(3.6)
    groups=[("OPPORTUNITY","Gap + historical access\n+ feasible route",GREEN),
            ("STOLEN PROPERTY","Counterfoil + packet\n+ verified custody",GOLD),
            ("DATED PHYSICAL LINK","Logged part + matching catch\n+ before/after observation",RED)]
    for i,(title,body,c) in enumerate(groups):
        xx=.22+i*3.32
        box(ax,xx,1.87,2.88,1.3,title,body,edge=c,size=9.6)
        arrow(ax,(xx+1.44,1.81),(5,1.18),c)
    box(ax,2.65,.23,4.7,.92,"SUBMIT ONLY WHAT THE LINKS SUPPORT","Partial findings remain valid outcomes",color=SAND,size=10)
    save(fig,"evidence_chain")

    fig,ax=canvas(3.55)
    for i,(title,body) in enumerate([
        ("H · LOFT RACK","Start of day 1\nEarly supervised search"),
        ("CORIN'S WALLET","Day 1 · Lamplight\nObserve the retrieval"),
        ("PRIVATE DESK","After retrieval\nConsent or limited search")]):
        xx=.2+i*3.34
        box(ax,xx,1.52,2.78,1.45,title,body,size=10)
        if i<2: arrow(ax,(xx+2.82,2.25),(xx+3.27,2.25))
        arrow(ax,(xx+1.39,1.47),(5,.93),GOLD)
    box(ax,2.73,.15,4.54,.74,"RECOVER → AUTHENTICATE → LODGE","",color=SAND,size=10)
    label(ax,5,3.28,"ONE PACKET · NO AUTOMATIC DESTRUCTION",11,GREEN,"bold")
    save(fig,"packet_custody")

    fig,ax=canvas(4.05)
    box(ax,.22,.25,9.56,3.53,"",color=PAPER)
    label(ax,.55,3.47,"THE COUNTER ALIBI",13,GREEN,"bold",ha="left")
    ax.plot([.55,9.45],[3.13]*2,color=LINE)
    rows=[("HEARD DIRECTLY", "Lise: I saw him before the lift and when it finished.",GREEN),
          ("OBSERVED", "The counter is hidden from her place in the yard.",GREEN),
          ("CLAIM", "Corin: I never left the counter.",GOLD),
          ("MY HYPOTHESIS", "He may have used the service connection in between.",MUTED)]
    for i,(tag,body,c) in enumerate(rows):
        y=2.83-i*.55
        label(ax,.55,y,tag,9,c,"bold",ha="left")
        label(ax,3.05,y,body,10,INK,ha="left")
    ax.plot([.55,9.45],[.73]*2,color=LINE)
    label(ax,.55,.48,"Next known question: was that route available at the time?",10,GREEN,ha="left")
    save(fig,"notebook")

    fig,ax=canvas(2.45)
    box(ax,.2,1.06,2.12,.96,"OPEN INQUIRY","Acquire and inspect",size=11)
    box(ax,3.77,1.06,2.4,.96,"SUBMISSION","A bounded request",size=11)
    box(ax,7.53,1.06,2.2,.96,"FINDING","Supported claims",size=11)
    arrow(ax,(2.38,1.53),(3.72,1.53)); arrow(ax,(6.22,1.53),(7.48,1.53))
    arrow(ax,(8.63,.98),(1.28,.98),GOLD,rad=-.1)
    label(ax,5,.39,"New evidence can reopen or extend the record",11,GOLD)
    label(ax,5,2.27,"SEPARATE FINDINGS · ONE CONTINUING INQUIRY",11,GREEN,"bold")
    save(fig,"case_states")
