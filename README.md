# rust-migration-thesis
This is a work-in-progress repository of Jort van Driel's work on his Master's thesis on the migration of software projects to Rust.

## Research questions

### Main research question
> In open-source software projects, how does migration to Rust affect reduction of technical debt?

(Notes: "affect" is still broadly interpretable and implies causality, maybe use "relate to a"?)

### Subquestion 1
> Which open-source projects have migrated or are in the process of migrating from another programming language to Rust?

(Notes: should question include extend?)

#### Steps
- Find projects using data mining: create or re-use dataset
- Q.A: Definition of migration? -> literature review
- Q.B: Do I include heuristics (/categorization)? If yes ->
  - Q.C: What are relevant heuristics -> literature review
  - Find values for heuristics using data mining
- Q.D: Do I already separate different projects by how they migrate (eg fully or partially, are as replacement or as addition, official or unofficial)? -> ?

### Subquestion 2
> What types of technical debt or other factors cause these projects to migrate?

(Notes: "or other factors" feels weird, but using only technical debt might miss developer stated reason. Also "cause" implies causality)

#### Steps
- Q.A: What kinds of technical debt do we define and how can these be measured? -> literature review
- Q.B: How are these technical debts present in the selected repositories before migration? -> measurements
- Qualitative interview with project (lead) developers

### Subquestion 3
> What strategies are used during the migration process?

(Notes: still broad. Should I include "... to reduce technical debt"?)

#### Steps
- Q.A: What are often used strategies? -> literature review
- Q.B: How do I find strategies based on code? -> ?
- Qualitative interview with project (lead) developers
- Analyze per heuristic

### Subquestion 4
> For projects that completed migration, to what extent is technical debt reduced?

(Notes: Should I keep using positive outcome "reduced" or should it all be neutrally phrased?)

#### Steps
- Q.A: How do I select a before and after point in time (commit) to analyze? -> ?
- Q.B: How are these technical debts present in the selected repositories after migration? -> measurements
- Q.C: Are there significant reductions? -> statistical analysis
- Analyze per heuristic
- Analyze per strategy

### Questions missing a method to find answer
- Q1.B Important to answer early in phase 1
- Q1.D Important to answer early in phase 1 
- Q3.B Can be answered later, but before interviews in phase 2
- Q4.A Can be answered later, but before phase 2

## Relevance
Many (open-source) software projects have migrated to or are migrating to Rust. Notable examples include the Rust _uutils_ implementation of _GNU coreutils_ and Firefox's _Oxidation_ iniative, in which parts of Firefox are being migrated to Rust. Recent work, such as the November issue of _Communications of the ACM_, highlights tools for automatically translating C to Rust. However, organizations also adopt other strategies, such as step-by-step migration. 

This thesis aims to examine which migration strategies are used and how effective they are at reducing technical debt. I will conduct a quantitative analysis of technical debt before and after migration, using heuristics derived from code analysis. Additionally, I will conduct qualitative interviews to gain an (empirical) understanding of the developers' strategies and experiences.

The findings will help organizations and developers determine whether migrating to Rust may be beneficial in their case and, if so, which strategies most effectively reduce technical debt. In terms of novelty, I could not find any previous research broadly examining migration to Rust.

## Phase planning

### Phase 1 - deadline: first stage review (9 Feb)
The goals of this phase are to:
- Literature review of a) data mining for software repositories and relevant heuristics, b) software migration definition and it's strategies and c) technical debt types and measurable heuristics.
- Select software repository heuristics and technical debt heuristics
- Finding open-source projects that have migrated or are migrating using data mining
- Obtain software repository heuristics measurements
- Researching ethical considerations when reaching out and for do's/don'ts in interviews
- Reaching out to (lead) developers/organizations of these open-source projects

### Phase 2 - deadline: 4 weeks before greenlight review (15 Apr)
The goals of this phase are to:
- Select before and after migration point in time per repository
- Obtain technical debt heuristic measurements from before and after migration
- Hold interviews with (lead) developers/organizations
- Classify strategy using heuristics and/or interview result
- Obtain other empirically interesting measurements based on interview results or further findings

### Phase 3 - deadline: greenlight review (13 May)
The goals of this phase are to:
- Statistical analysis of technical debt reduction per project heuristic and per strategy
- Analyze implications, future work, etc.
- Complete report
