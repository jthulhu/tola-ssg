// `collection` imports this file inside each operation, so these helpers never become exports.

#let require(condition, message: none) = {
  if not condition {
    import "@tola/host:0.0.0": report-error
    report-error(message)
  }
}

#let memberships-of(member, keys, operation) = {
  let memberships = keys(member)
  require(type(memberships) == array, message: operation + " `keys` must return an array; found `" + repr(type(memberships)) + "`")
  memberships
}

#let duplicate-key-message(operation, member-key) = {
  operation + " requires unique keys; `" + repr(member-key) + "` occurs more than once"
}

#let unique-keys(members, key, operation) = {
  require(type(members) == array, message: operation + " `members` must be an array; found `" + repr(type(members)) + "`")
  let member-keys = members.map(key)
  let seen = ()
  for member-key in member-keys {
    if member-key in seen {
      require(false, message: duplicate-key-message(operation, member-key))
    }
    seen.push(member-key)
  }
  member-keys
}

#let hierarchy-keys(members, key, operation) = {
  let member-keys = unique-keys(members, key, operation)
  require(none not in member-keys, message: operation + " hierarchy keys cannot be `none`")
  member-keys
}

#let position-of(members, at, key, operation) = {
  unique-keys(members, key, operation).position(member-key => member-key == at)
}

#let parent-key(origin, parent, operation) = {
  let ancestor = parent(origin)
  if ancestor == origin {
    require(false, message: operation + " parent chain contains a cycle at key `" + repr(origin) + "`")
  }
  ancestor
}

#let parent-chain(origin, parent, operation) = {
  let chain = ()
  let ancestor = parent-key(origin, parent, operation)
  while ancestor != none {
    if ancestor == origin or ancestor in chain {
      require(false, message: operation + " parent chain contains a cycle at key `" + repr(ancestor) + "`")
    }
    chain.push(ancestor)
    ancestor = parent-key(ancestor, parent, operation)
  }
  chain
}

#let ancestor-members(members, member-keys, at, parent, operation) = {
  let ancestors = ()
  for ancestor in parent-chain(at, parent, operation).rev() {
    let position = member-keys.position(member-key => member-key == ancestor)
    if position != none { ancestors.push(members.at(position)) }
  }
  ancestors
}
