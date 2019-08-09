OBJDIR = artifacts

all: package

package: build
		ar qs $(OBJDIR)/libtzmock.a $(OBJDIR)/*.o

build: build-deps
		cd tezos && opam config exec -- make
		mkdir -p $(OBJDIR)
		mv tezos/libtezos-protocol-ffi.so $(OBJDIR)/libtzreal.o

build-deps: git
		cd tezos && make build-deps

git:
		@if [ ! -d tezos ] ;\
		then \
	  		echo "Tezos repo does not exist, fetching"; \
	  		git clone https://gitlab.com/jurajselep/tezos.git; \
	  		cd tezos && git checkout poc-protocol-alphanet; \
		fi

clean:
		rm -rf tezos artifacts
