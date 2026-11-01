# Tickets

Tickets tracks work for a human and agents through projects, tickets, labels and
dependencies. This glossary defines the shared domain language for that work.

## Language

**Project**:
A named collection of tickets with its own ticket-key prefix and numbering.
Archiving a project hides it from active navigation while retaining its tickets.

**Ticket**:
A piece of work belonging to one project, with a title, description, status and
optional priority, assignee, labels and dependencies.
_Avoid_: Issue (except when referring to a source GitHub issue).

**Ticket key**:
A ticket's human-readable reference, formed from its project's prefix and its
number within that project, such as TIK-113. It can change when the project prefix
changes or the ticket moves to another project; it is not a permanent identity.

**Status**:
A ticket's workflow stage: Backlog, Todo, In Progress, In Review, Done or Canceled.
Blocked is a separate derived condition, not another status.

**Label**:
A named, colored classification that can be attached to tickets across projects.

**Dependency**:
A directed relationship from a blocker ticket to the ticket it blocks.
Dependencies cannot point to the same ticket or form cycles.

**Blocker**:
The ticket at the source of a dependency, whose resolution the blocked ticket
depends on. A blocker is unresolved while its status is neither Done nor Canceled.

**Blocked**:
The derived condition of a ticket with at least one unresolved blocker.
It does not replace the ticket's status and clears when every blocker is Done or
Canceled.
_Avoid_: Blocked status.

**Pickup rule**:
A policy governing which Todo tickets are eligible for pickup, their ranking and
the limits on claimed work. Workspace rules apply unless a project has its own
override.

**Pickup claim**:
Temporary ownership of a ticket by the authenticated human or agent that picks it
up, ending on release or expiry. Claim ownership is distinct from the ticket's
assignee and does not assign the ticket.
_Avoid_: Assignment (for claim ownership).

**Ticket history**:
The record of observed ticket changes, including who made them and the previous
and new values. It does not reconstruct unobserved past events from timestamps
or a ticket's current state.

**Pull request link**:
An association between a ticket and a pull request, carrying either closing or
reference-only intent. Both can reflect progress in the ticket's status, but only
a closing link marks it Done on merge, and Canceled tickets remain Canceled.

**Imported ticket**:
An editable native ticket brought in from a GitHub issue, retaining its source
identity, source URL and imported comment provenance markers.
_Avoid_: Mirror (which suggests ongoing synchronization or edit restrictions).
